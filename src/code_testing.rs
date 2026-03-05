// Copyright (c) 2025 Shenghao Yang. All rights reserved.
// Licensed under the MIT License. See LICENSE-MIT for details.

//! Testing utilities for fountain codes
//! 
//! This module provides generic testing functions for any code scheme implementation.

use fountain_engine::*;
use crate::{VecDataOperater, operation_counter::PerformanceMetrics};
use rand::prelude::SliceRandom;
use std::time::Instant;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, BufReader, BufRead, Write};
use serde::{Serialize, Deserialize};

/// Results from a code scheme test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Number of source symbols
    pub k: usize,
    /// Whether decoding was successful
    pub num_mismatches: usize,
    /// Precoding metrics
    pub precoding_metrics: PerformanceMetrics,
    /// Encoding metrics
    pub encoding_metrics: PerformanceMetrics,
    /// Decoding metrics
    pub decoding_metrics: PerformanceMetrics,
    /// Encoding time in milliseconds
    pub precoding_time_ms: f64,
    /// Encoding time in milliseconds
    pub encoding_time_ms: f64,
    /// Decoding time in milliseconds
    pub decoding_time_ms: f64,
}

/// Save test results to a file in JSON Lines format (one JSON object per line)
pub fn save_test_results(results: &[TestResult], filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(filename)?;
    let mut writer = BufWriter::new(file);
    for result in results {
        let json = serde_json::to_string(result)?;
        writeln!(writer, "{}", json)?;
    }
    writer.flush()?;
    Ok(())
}

/// Load test results from a file in JSON Lines format (one JSON object per line)
pub fn load_test_results(filename: &str) -> Result<Vec<TestResult>, Box<dyn std::error::Error>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut results = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if !line.trim().is_empty() {
            let result: TestResult = serde_json::from_str(&line)?;
            results.push(result);
        }
    }
    Ok(results)
}

/// Test a code scheme multiple times and collect results
pub fn test_code_scheme_multiple<C>(num_runs: usize, code_scheme: &C, k: usize, num_coded_vectors: usize) -> Vec<TestResult>
where
    C: CodeScheme + Clone,
{
    (0..num_runs).map(|_i| {
        test_code_scheme(code_scheme, k, num_coded_vectors)
    }).collect()
}

/// Test a code scheme without actual data vectors
pub fn test_code_scheme<C>(code_scheme: &C, k: usize, num_coded_vectors: usize) -> TestResult
where
    C: CodeScheme + Clone,
{    
    // Encoding
    let params = code_scheme.get_params();

    let mut coded_ids : Vec<usize> = if code_scheme.code_type() == CodeType::Systematic {
        let mut ids: Vec<usize> = (0..k).collect();
        ids.extend(params.num_total()..params.num_total() + num_coded_vectors-k);
        ids
    } else {
        (params.num_total()..params.num_total() + num_coded_vectors).collect()
    };
    
    // Precoding
    let precoding_time = Instant::now();
    let mut encoder = Encoder::new(code_scheme.clone());
    let precoding_time = precoding_time.elapsed();
    let precoding_metrics = PerformanceMetrics
        ::from_operations(&encoder.manager.move_new_operations(), params.k);

    // LT Encoding
    let encoding_time = Instant::now();
    for coded_id in &coded_ids {
        encoder.encode_coded_vector(*coded_id);
    }
    let encoding_time = encoding_time.elapsed();
    let encoding_metrics = PerformanceMetrics
        ::from_operations(&encoder.manager.move_new_operations(), params.num_total());
    
    // enumulate packet loss by randomly shuffling the coded_ids
    let mut rng = rand::thread_rng();
    coded_ids.shuffle(&mut rng);

    // Decoding
    let mut decoded_successfully = false;
    //let mut vectors_used = 0;    
    let decoding_time = Instant::now();
    let mut decoder = Decoder::new(code_scheme.clone());
    // start decoding
    for coded_id in coded_ids.iter() {
        let status = decoder.add_coded_id(*coded_id);        
        if matches!(status, DecodeStatus::Decoded) {
            decoded_successfully = true;
            //vectors_used = i + 1;
            break;
        }
    }
    let decoding_time = decoding_time.elapsed();
    let decoding_metrics = PerformanceMetrics
        ::from_operations(&decoder.manager.move_new_operations(), decoder.manager.coded_vector_inserted);
    
    let num_mismatches = if decoded_successfully {
        0
    } else {
        k
    };
    
    TestResult {
        k,
        num_mismatches,
        precoding_metrics,
        encoding_metrics,
        decoding_metrics,
        precoding_time_ms: precoding_time.as_secs_f64() * 1000.0,
        encoding_time_ms: encoding_time.as_secs_f64() * 1000.0,
        decoding_time_ms: decoding_time.as_secs_f64() * 1000.0,
    }
}

/// Test a code scheme with actual message vectors (full correctness testing)
pub fn test_code_scheme_with_data_vectors<C>(code_scheme: &C, k: usize, data_vector_length: usize, num_coded_vectors: usize) -> TestResult
where
    C: CodeScheme + Clone,
{    
    // Create test data
    let mut message_vectors = vec![vec![0u8; data_vector_length]; k];
    for i in 0..k {
        for j in 0..data_vector_length {
            message_vectors[i][j] = ((i * 7 + j * 13) % 256) as u8;
        }
    }
    
    // Setup encoder data manager
    let mut encode_data_operater = VecDataOperater::new(data_vector_length);
    for (i, vector) in message_vectors.iter().enumerate() {
        encode_data_operater.insert_vector(vector, i);
    }
    
    // Encoding
    let params = code_scheme.get_params();
    let mut coded_ids : Vec<usize> = if code_scheme.code_type() == CodeType::Systematic {
        let mut ids: Vec<usize> = (0..k).collect();
        ids.extend(params.num_total()..params.num_total() + num_coded_vectors-k);
        ids
    } else {
        (params.num_total()..params.num_total() + num_coded_vectors).collect()
    };

    // Precoding
    let precoding_time = Instant::now();
    let mut encoder = Encoder::new_with_operator(code_scheme.clone(), Box::new(encode_data_operater));
    let precoding_time = precoding_time.elapsed();
    let precoding_metrics = PerformanceMetrics
        ::from_operations(&encoder.manager.move_new_operations(), params.k);

    // LT Encoding
    let encoding_time = Instant::now();
    let mut coded_id_to_data_id: HashMap<usize, usize> = HashMap::new();
    for coded_id in &coded_ids {
        if let Some(data_id) = encoder.encode_coded_vector(*coded_id) {
            coded_id_to_data_id.insert(*coded_id, data_id);
        }
    }
    let encoding_time = encoding_time.elapsed();
    let encoding_metrics = PerformanceMetrics
        ::from_operations(&encoder.manager.move_new_operations(), params.num_total());
    
    let encoder_operator = encoder.manager.move_operator();

    // enumulate packet loss by randomly shuffling the coded_ids
    let mut rng = rand::thread_rng();
    coded_ids.shuffle(&mut rng);
    
    // Decoding
    let mut decoded_successfully = false;
    //let mut vectors_used = 0;
    let decoding_time = Instant::now();
    let mut decoder = Decoder::new_with_operator(code_scheme.clone(), Box::new(VecDataOperater::new(data_vector_length)));
    for coded_id in coded_ids.iter() {
        if let Some(data_id) = coded_id_to_data_id.get(coded_id) {
            let status = decoder.add_coded_vector(*coded_id, encoder_operator.get_vector(*data_id));
            if matches!(status, DecodeStatus::Decoded) {
                decoded_successfully = true;
                //vectors_used = i + 1;
                break;
            }
        }
    }
    let decoding_time = decoding_time.elapsed();
    let decoding_metrics = PerformanceMetrics
        ::from_operations(&decoder.manager.move_new_operations(), decoder.manager.coded_vector_inserted);

    // Verify decoded vectors match message vectors
    let decoder_operator = decoder.manager.move_operator();
    let num_mismatches = if decoded_successfully {
        (0..k).filter(|&i| decoder_operator.get_vector(i) != message_vectors[i]).count()
    } else {
        k
    };
    
    
    TestResult {
        k,
        num_mismatches,
        precoding_metrics,
        encoding_metrics,
        decoding_metrics,
        precoding_time_ms: precoding_time.as_secs_f64() * 1000.0,
        encoding_time_ms: encoding_time.as_secs_f64() * 1000.0,
        decoding_time_ms: decoding_time.as_secs_f64() * 1000.0,
    }
}



