// Copyright (c) 2025 Shenghao Yang. All rights reserved.
// Licensed under the MIT License. See LICENSE-MIT for details.

//! Fountain Utility Library
//!
//! This library provides operation counting and reporting tools for fountain codes.
//! It includes operation counting and text-based reporting capabilities to help
//! evaluate and analyze fountain code implementations.

#![allow(
    clippy::needless_range_loop,
)]

pub mod operation_counter;
pub mod testing_statistics;
pub mod code_testing;
/// In-memory data operator storing vectors in `Vec<Vec<u8>>`; implements the `DataOperator` trait.
pub mod vec_data_operater;
pub mod io_data_operator;

pub use operation_counter::*;
pub use testing_statistics::*;
pub use code_testing::*;
pub use vec_data_operater::*;
pub use io_data_operator::*;
