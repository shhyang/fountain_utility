# Changelog

All notable changes to the published `fountain_utility` crate are documented here.

## [2.0.0] - 2026-07-17

### Changed

- **`fountain_engine` dependency** bumped to **2.0.0** (SystemSolver-based engine; no published `profiling` / legacy solver features).
- This major version tracks the engine 2.x cutover. The utility API surface is unchanged for typical `VecDataOperater` / padding / testing use.


## [1.3.1] - 2026-06-19

### Changed

- **`fountain_engine` dependency** bumped to **1.3.2** (`Operation::EnsureZeroOne`, `HDPC::lu_idssh`, precoding refactor).
- **`PerformanceMetrics::from_operations`** counts `EnsureZeroOne` toward `max_storage`.
- **`VecDataOperater::execute`** replays `EnsureZeroOne` operations.

## [1.3.0] - 2026-06-08

### Added

- **`padding_codec` module** — generic padding-aware wrappers that delegate to `fountain_engine` v1.3.1 native padding (`new_with_num_source` / `install_padding`):
  - `BlockSizePolicy` — trait for schemes where application source count K ≤ internal block size K′.
  - `WithSourceK` — adapts any `CodeScheme` to a smaller `source_k` without changing `CodeParams.k`.
  - `PaddedEncoder` / `PaddedDecoder` — thin wrappers around `Encoder` / `Decoder` with `source_symbols`, `block_symbols`, and `num_padding` helpers.
  - `PaddedRealSymbolSession` — `RealSymbolSession` implementation for deferred encode/decode benchmarks with padding.

  Example (scheme implements `BlockSizePolicy`):

  ```rust
  let enc = PaddedEncoder::new(scheme.clone());
  let dec = PaddedDecoder::new(scheme);
  ```

  Or adapt an existing scheme:

  ```rust
  let scheme = WithSourceK::new(raptor_q_scheme, k_app);
  let enc = PaddedEncoder::new(scheme);
  ```

- Unit and integration tests in `padding_codec` (ordinary padded round-trip via `WithSourceK` + `HDPCLTCode`).

### Changed

- **`fountain_engine` dependency** bumped to **1.3.1** (native padding API on `DataManager` and `Solver`).
- **`lib.rs`** exports `padding_codec` and `real_symbol_benchmark` at the crate root.
