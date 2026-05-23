# Development Setup

This page walks you through setting up a local development environment for SlateDuck. The process is straightforward: install Rust, clone the repository, and run the tests.

## Prerequisites

- **Rust 1.80+** (install via [rustup.rs](https://rustup.rs))
- **Git** (for cloning the repository)
- **A C compiler** (for building some native dependencies — gcc/clang on Linux, Xcode Command Line Tools on macOS)

Optional but recommended:
- **Docker** (for running integration tests with MinIO)
- **DuckDB** (for end-to-end testing with the ducklake extension)
- **Python 3.10+** (for documentation builds)

## Getting Started

```bash
# Clone the repository
git clone https://github.com/slateduck/slateduck.git
cd slateduck

# Build all crates
cargo build

# Run the test suite
cargo test

# Run with local filesystem storage (development mode)
cargo run -- --storage ./dev-catalog --bind 127.0.0.1:5432
```

## Workspace Structure

SlateDuck is a Cargo workspace with these crates:

| Crate | Purpose |
|-------|---------|
| `slateduck-core` | Foundation types, key encoding, value encoding, MVCC |
| `slateduck-catalog` | Persistence layer, read/write operations, GC |
| `slateduck-sql` | SQL statement classifier |
| `slateduck-pgwire` | PostgreSQL wire protocol server |
| `slateduck-ffi` | C FFI for native DuckDB extension |
| `slateduck-datafusion` | DataFusion catalog provider |
| `slateduck-sqlite-vfs` | SQLite VFS layer (experimental) |

Dependencies flow upward: `core` → `catalog` → `sql` → `pgwire` (the binary).

## Running Specific Tests

```bash
# Core crate tests
cargo test -p slateduck-core

# Catalog integration tests
cargo test -p slateduck-catalog --test integration_tests

# Property-based tests
cargo test -p slateduck-core --test property_tests

# Benchmarks
cargo bench -p slateduck-catalog
```

## Development Workflow

1. Create a feature branch: `git checkout -b feature/my-change`
2. Make your changes
3. Run `cargo fmt` to format code
4. Run `cargo clippy` to check for linting issues
5. Run `cargo test` to verify all tests pass
6. Commit with a conventional commit message: `feat: add X` or `fix: correct Y`
7. Push and open a pull request

## Documentation Development

```bash
# Install documentation dependencies
pip install -r requirements-docs.txt

# Serve documentation locally (with hot reload)
mkdocs serve

# Build documentation (for verifying production build)
mkdocs build --strict
```
