# Contributing to SlateDuck

Thank you for your interest in contributing to SlateDuck!

## Development Setup

1. Install Rust (1.75+): https://rustup.rs/
2. Clone the repository:
   ```bash
   git clone https://github.com/trickle-labs/slateduck.git
   cd slateduck
   ```
3. Run tests:
   ```bash
   cargo test --all
   ```
4. Check formatting and lints:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   ```

## Workspace Structure

```
slateduck/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── slateduck-core/        # Key layout, encoding, tags, error types
│   ├── slateduck-catalog/     # DuckLake catalog operations (v0.2)
│   ├── slateduck-sql/         # SQL dispatcher for pgwire (v0.3)
│   ├── slateduck-sqlite-vfs/  # SQLite VFS layer (future)
│   ├── slateduck-pgwire/      # PG wire protocol sidecar (v0.3)
│   └── slateduck-ffi/         # C FFI bindings (v0.5)
├── docs/                      # Architecture and phase documents
├── tests/                     # Integration test fixtures
└── extension/                 # DuckDB extension (v0.5)
```

## Pull Request Process

1. Create a feature branch from `master`
2. Ensure all tests pass: `cargo test --all`
3. Ensure no clippy warnings: `cargo clippy --all-targets --all-features`
4. Ensure proper formatting: `cargo fmt --all`
5. Open a pull request against `master`

## Code Style

- Follow standard Rust conventions
- Use `thiserror` for error types
- All public APIs must have doc comments
- Integration tests go in `tests/`; unit tests use `#[cfg(test)] mod tests`

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
