# Code Style

This page documents the coding conventions used in SlateDuck. Following these conventions ensures consistency across the codebase and makes code review faster.

## Formatting

All Rust code must pass `cargo fmt` with default settings. This is enforced in CI. Do not configure custom formatting rules.

## Linting

All code must pass `cargo clippy` with no warnings. Common clippy suggestions are generally good advice — follow them unless there is a specific technical reason not to.

## Naming Conventions

- **Types:** PascalCase (`CatalogStore`, `WriterEpoch`, `StatementKind`)
- **Functions:** snake_case (`classify_statement`, `write_batch`, `advance_retention`)
- **Constants:** SCREAMING_SNAKE_CASE (`ABI_VERSION`, `MAGIC_BYTES`, `MAX_KEY_SIZE`)
- **Modules:** snake_case matching the filename (`catalog_provider.rs` → `mod catalog_provider`)

## Error Handling

- Return `Result<T, E>` from all fallible operations
- Use the crate's error enum (e.g., `CatalogError`, `PgWireError`)
- Include context in error messages: "failed to read schema {schema_id}: {source}"
- Do not use `.unwrap()` or `.expect()` in library code unless the invariant is documented and provably upheld

## Comments

- Write doc comments (`///`) for all public types and functions
- Prefer self-documenting code over inline comments
- Use `// TODO:` for planned improvements (include a tracking issue if available)
- Do not write comments that merely repeat what the code says

## Module Organization

- One public type per file (for significant types)
- Keep files under 500 lines where possible
- Use `mod.rs` for modules with multiple subfiles
- Put tests in a `#[cfg(test)] mod tests` block at the bottom of each file, and integration tests in `tests/`

## Dependency Policy

- Prefer well-maintained crates with active ownership
- Pin to compatible version ranges (e.g., `"0.13"` not `"*"`)
- Minimize the dependency tree — do not add crates for trivial functionality
- All new dependencies must be justified in the PR description

## Commit Messages

Follow Conventional Commits:

```
feat: add support for ALTER TABLE SET SCHEMA
fix: correct MVCC visibility for snapshot 0
docs: update deployment guide for Kubernetes
refactor: extract key encoding into separate module
test: add wire corpus entries for DuckDB 1.3.0
```

The scope is optional but appreciated for large changes: `feat(catalog): add secondary index support`
