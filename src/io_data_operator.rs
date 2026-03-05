// Copyright (c) 2025 Shenghao Yang. All rights reserved.
// Licensed under the MIT License. See LICENSE-MIT for details.

//! I/O Data Operator
//!
//! This module provides a DataOperator implementation that writes operations
//! to a flexibly configured I/O writer. This is useful for logging, debugging,
//! or sending operations to external systems.

use fountain_engine::traits::DataOperator;
use fountain_engine::types::Operation;
use std::io::Write;

/// A DataOperator that writes operations to a configurable writer.
/// 
/// This operator logs all operations to the writer but does not execute them.
/// It's useful for logging, debugging, or sending operations to external systems.
/// 
/// # Example
/// 
/// ```no_run
/// use fountain_utility::io_data_operator::IoDataOperator;
/// use std::io::stdout;
/// 
/// let mut io_op = IoDataOperator::new(stdout());
/// ```
pub struct IoDataOperator<W: Write> {
    writer: W,
    format: OperationFormat,
}

/// Format options for how operations are written to the writer
#[derive(Clone, Copy, Debug)]
pub enum OperationFormat {
    /// Human-readable text format (using Debug)
    Debug,
    /// Custom format string (not yet implemented)
    Custom,
}

impl<W: Write> IoDataOperator<W> {
    /// Create a new IoDataOperator that writes to the given writer.
    /// Operations are only logged, not executed.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            format: OperationFormat::Debug,
        }
    }

    /// Set the format for writing operations
    pub fn set_format(&mut self, format: OperationFormat) {
        self.format = format;
    }

    /// Write an operation to the writer based on the configured format
    fn write_operation(&mut self, operation: &Operation) -> std::io::Result<()> {
        match self.format {
            OperationFormat::Debug => {
                writeln!(self.writer, "{:?}", operation)?;
            }
            OperationFormat::Custom => {
                // For now, fall back to Debug format
                writeln!(self.writer, "{:?}", operation)?;
            }
        }
        Ok(())
    }
}

impl<W: Write> DataOperator for IoDataOperator<W> {

    fn execute(&mut self, operation: &Operation) {
        // Write the operation to the writer
        let _ = self.write_operation(operation);
    }
}

impl IoDataOperator<std::io::Stdout> {
    /// Create an IoDataOperator that writes to stdout (like DisplayDataOperator)
    pub fn stdout() -> Self {
        Self::new(std::io::stdout())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_io_data_operator_basic() {
        let mut buffer = Vec::new();
        {
            let mut io_op = IoDataOperator::new(Cursor::new(&mut buffer));

            io_op.execute(&Operation::EnsureZero {
                list_id: vec![1, 2, 3],
            });
            // Flush the writer
            let _ = io_op.writer.flush();
        }

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("EnsureZero"));
        assert!(output.contains("1"));
        assert!(output.contains("2"));
        assert!(output.contains("3"));
    }
}

