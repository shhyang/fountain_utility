// Copyright (c) 2025 Shenghao Yang. All rights reserved.
// Licensed under the MIT License. See LICENSE-MIT for details.

//! Statistical analysis for fountain code test results
//! 
//! This module provides tools for collecting and analyzing statistics from
//! multiple test runs of the same code scheme.

use crate::code_testing::TestResult;
use serde::{Serialize, Deserialize};

/// Statistics collected from multiple test runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStatistics;
/* {
    /// Number of test runs
    pub num_runs: usize,
    /// Number of successful decodings
    pub successful_runs: usize,
    /// Success rate (successful_runs / num_runs)
    pub success_rate: f64,
    /// Coding overhead statistics
    pub overhead_stats: Statistics,
    /// Storage statistics
    pub storage_stats: Statistics,
    /// Operation statistics
    pub precoding_avg_comp: AverageComputation,
    pub encoding_avg_comp: AverageComputation,
    pub decoding_avg_comp: AverageComputation,
    /// Average precoding time in milliseconds
    pub engine_avg_time: AverageTime,
}
*/
/// Statistics for coding overhead
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    /// Arithmetic mean of the values
    pub mean: f64,
    /// Minimum value observed
    pub min: f64,
    /// Maximum value observed
    pub max: f64,
}

impl Default for Statistics {
    fn default() -> Self {
        Self {
            mean: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }
}

/// Statistics for computation costs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AverageComputation {
    /// Average count of multiply-by-alpha operations
    pub multiply_alpha: f64,
    /// Average count of scalar multiplication operations
    pub multiply_scalar: f64,
    /// Average count of vector addition operations
    pub vector_add: f64,
    /// Average count of multiply-add (axpy) operations
    pub mul_add: f64,
}

/// Statistics for time costs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AverageTime {
    /// Average precoding phase time in milliseconds
    pub precoding: f64,
    /// Average encoding phase time in milliseconds
    pub encoding: f64,
    /// Average decoding phase time in milliseconds
    pub decoding: f64,
}

impl Statistics {
    fn from_values(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        Self { mean, min, max }
    }
}

impl TestStatistics {
    /// Get success rate statistics
    pub fn success_rate(results: &[TestResult]) -> (usize, usize, f64) {
        let num_runs = results.len();
        if num_runs == 0 {
            return (0, 0, 0.0);
        }
        
        let succ_results: Vec<&TestResult> = results.iter().filter(|r| r.num_mismatches == 0).collect();
        let successful_runs = succ_results.len();
        let success_rate = successful_runs as f64 / num_runs as f64;
        (num_runs, successful_runs, success_rate)
    }

    /// Calculate overhead statistics (decoding overhead = num_coded_vectors used - k)
    pub fn overhead_stats(k: usize, results: &[TestResult]) -> Statistics {
        let succ_results: Vec<&TestResult> = results.iter().filter(|r| r.num_mismatches == 0).collect();
        if succ_results.is_empty() {
            return Statistics::default();
        }
        
        //let k = self.k; // All results should have the same k
        let overheads: Vec<f64> = succ_results.iter()
            .map(|r| r.decoding_metrics.num_coded_vectors as f64 - k as f64)
            .collect();
        Statistics::from_values(&overheads)
    }

    /// Calculate storage statistics (max_storage from decoding)
    pub fn storage_stats(results: &[TestResult]) -> Statistics {
        let succ_results: Vec<&TestResult> = results.iter().filter(|r| r.num_mismatches == 0).collect();
        if succ_results.is_empty() {
            return Statistics::default();
        }
        
        let storages: Vec<f64> = succ_results.iter()
            .map(|r| r.decoding_metrics.max_storage as f64)
            .collect();
        Statistics::from_values(&storages)
    }

    /// Calculate average computation costs for precoding, encoding, and decoding
    pub fn avg_computation_costs(k: usize,results: &[TestResult]) -> (AverageComputation, AverageComputation, AverageComputation) {
        let succ_results: Vec<&TestResult> = results.iter().filter(|r| r.num_mismatches == 0).collect();
        if succ_results.is_empty() {
            return (AverageComputation::default(), AverageComputation::default(), AverageComputation::default());
        }
        
        //let k = succ_results[0].k; // All results should have the same k
        let count = succ_results.len() as f64;
        
        let precoding = succ_results.iter().fold(AverageComputation::default(), |acc, r| {
            AverageComputation {
                multiply_alpha: acc.multiply_alpha + r.precoding_metrics.multiply_alpha as f64,
                multiply_scalar: acc.multiply_scalar + r.precoding_metrics.multiply_scalar as f64,
                vector_add: acc.vector_add + r.precoding_metrics.vector_add as f64,
                mul_add: acc.mul_add + r.precoding_metrics.mul_add as f64,
            }
        });
        
        let encoding = succ_results.iter().fold(AverageComputation::default(), |acc, r| {
            let normalizer = r.encoding_metrics.num_coded_vectors as f64;
            AverageComputation {
                multiply_alpha: acc.multiply_alpha + r.encoding_metrics.multiply_alpha as f64 / normalizer,
                multiply_scalar: acc.multiply_scalar + r.encoding_metrics.multiply_scalar as f64 / normalizer,
                vector_add: acc.vector_add + r.encoding_metrics.vector_add as f64 / normalizer,
                mul_add: acc.mul_add + r.encoding_metrics.mul_add as f64 / normalizer,
            }
        });
        
        let decoding = succ_results.iter().fold(AverageComputation::default(), |acc, r| {
            AverageComputation {
                multiply_alpha: acc.multiply_alpha + r.decoding_metrics.multiply_alpha as f64,
                multiply_scalar: acc.multiply_scalar + r.decoding_metrics.multiply_scalar as f64,
                vector_add: acc.vector_add + r.decoding_metrics.vector_add as f64,
                mul_add: acc.mul_add + r.decoding_metrics.mul_add as f64,
            }
        });
        
        (
            AverageComputation {
                multiply_alpha: precoding.multiply_alpha / count / k as f64,
                multiply_scalar: precoding.multiply_scalar / count / k as f64,
                vector_add: precoding.vector_add / count / k as f64,
                mul_add: precoding.mul_add / count / k as f64,
            },
            AverageComputation {
                multiply_alpha: encoding.multiply_alpha / count,
                multiply_scalar: encoding.multiply_scalar / count,
                vector_add: encoding.vector_add / count,
                mul_add: encoding.mul_add / count,
            },
            AverageComputation {
                multiply_alpha: decoding.multiply_alpha / count / k as f64,
                multiply_scalar: decoding.multiply_scalar / count / k as f64,
                vector_add: decoding.vector_add / count / k as f64,
                mul_add: decoding.mul_add / count / k as f64,
            },
        )
    }

    /// Calculate average time costs for precoding, encoding, and decoding
    pub fn avg_time_costs(results: &[TestResult]) -> AverageTime {
        let succ_results: Vec<&TestResult> = results.iter().filter(|r| r.num_mismatches == 0).collect();
        if succ_results.is_empty() {
            return AverageTime::default();
        }
        
        let count = succ_results.len() as f64;
        let (total_precoding, total_encoding, total_decoding) = succ_results.iter()
            .fold((0.0, 0.0, 0.0), |(prec, enc, dec), r| {
                (prec + r.precoding_time_ms, enc + r.encoding_time_ms, dec + r.decoding_time_ms)
            });
        
        AverageTime {
            precoding: total_precoding / count,
            encoding: total_encoding / count,
            decoding: total_decoding / count,
        }
    }
}

