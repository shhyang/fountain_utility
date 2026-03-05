// Copyright (c) 2025 Shenghao Yang. All rights reserved.
// Licensed under the MIT License. See LICENSE-MIT for details.

//! Operation counting and analysis for fountain code performance evaluation

use fountain_engine::types::{Operation};
use serde::{Serialize, Deserialize};

/// Operation counters for tracking different types of operations in DataManager
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Number of multiply-by-alpha operations
    pub multiply_alpha: usize,
    /// Number of scalar multiplication operations
    pub multiply_scalar: usize,
    /// Number of vector addition operations
    pub vector_add: usize,
    /// Number of multiply-add (axpy) operations
    pub mul_add: usize,
    /// Peak number of vectors stored simultaneously
    pub max_storage: usize,
    /// Number of coded vectors produced or consumed
    pub num_coded_vectors: usize,
}

impl PerformanceMetrics {
    /// Count operations 
    pub fn from_operations(operations: &[Operation], coded_vector_inserted: usize) -> Self {
        let mut metrics = Self::default();
        let mut current_storage = coded_vector_inserted;
        for operation in operations {
            match operation {
                Operation::EnsureZero { list_id } => {
                    current_storage += list_id.len();
                    if current_storage > metrics.max_storage {
                        metrics.max_storage = current_storage;
                    }
                },
                Operation::MultiplyAlpha { .. } => metrics.multiply_alpha += 1,
                Operation::MultiplyScalar { .. } => metrics.multiply_scalar += 1,
                Operation::AddToVector { list_id, .. } => metrics.vector_add += list_id.len(),
                Operation::BroadcastAdd { target_ids, .. } => metrics.vector_add += target_ids.len(),
                Operation::MulAdd { .. } => {metrics.mul_add += 1},
                Operation::MoveTo { .. } => {},
                Operation::CopyTo { .. } => {
                    current_storage += 1;
                    if current_storage > metrics.max_storage {
                        metrics.max_storage = current_storage;
                    }
                },
                Operation::Remove { .. } => {
                    if let Some(new_value) = current_storage.checked_sub(1) {
                        current_storage = new_value;
                    } else {
                        //current_storage = 0;
                        panic!("Current storage is 0, cannot remove a vector");
                    }
                },
                Operation::InfoCodedVector { .. } => metrics.num_coded_vectors += 1,
            }
        }
        metrics
    }    
}

