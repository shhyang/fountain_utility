# fountain_utility

Data operators and testing utilities for fountain code libraries.

## Data Operators

- **`VecDataOperater`**: In-memory `DataOperator` that stores vectors in a `Vec<Vec<u8>>`. Suitable for testing and small-scale encoding/decoding.
- **`IoDataOperator`**: Logs all operations to a configurable `Write` destination. Useful for debugging and tracing.

## Testing Utilities

- **`test_code_scheme_with_data_vectors`**: End-to-end encode/decode test for any `CodeScheme` with actual data verification.
- **`test_code_scheme_multiple`**: Run multiple trials and collect statistics.
- **`TestStatistics`**: Compute success rates, overhead statistics, and more.

## License

MIT License. See [LICENSE-MIT](LICENSE-MIT).

Copyright (c) 2025 Shenghao Yang. All rights reserved.
