# Testing

SlateDuck has a multi-layered testing strategy that ensures correctness from individual functions up to full protocol interactions. This page describes the testing philosophy, available test types, and how to write effective tests.

## Testing Philosophy

- **Every bug fix must have a regression test.** If a bug was found, it means our tests were insufficient. The fix includes a test that would have caught it.
- **Property-based tests for encoders.** Key encoding, value encoding, and MVCC visibility are tested with random inputs to find edge cases that example-based tests miss.
- **Wire corpus for protocol compatibility.** Real DuckDB output is captured and used as test fixtures. This catches regressions that synthetic tests might miss.
- **Integration tests for workflows.** End-to-end tests verify that complete operations (create schema, create table, insert data, query) work correctly through the full stack.

## Test Types

### Unit Tests

Located in `#[cfg(test)] mod tests` blocks within source files. Test individual functions in isolation:

```rust
#[test]
fn test_schema_key_roundtrip() {
    let key = SchemaKey::new(42, 100);
    let bytes = key.encode();
    let decoded = SchemaKey::decode(&bytes).unwrap();
    assert_eq!(key, decoded);
}
```

### Property-Based Tests

Located in `crates/slateduck-core/tests/property_tests.rs`. Use `proptest` to generate random inputs:

```rust
proptest! {
    #[test]
    fn key_encoding_preserves_order(a: u64, b: u64) {
        let key_a = encode_u64(a);
        let key_b = encode_u64(b);
        assert_eq!(a.cmp(&b), key_a.cmp(&key_b));
    }
}
```

### Integration Tests

Located in `crates/*/tests/`. Test complete operations through the public API:

```rust
#[tokio::test]
async fn test_create_table_and_list_columns() {
    let store = CatalogStore::open_temp().await.unwrap();
    let writer = store.writer().await.unwrap();
    // ... create schema, table, columns ...
    let reader = store.reader().await.unwrap();
    let columns = reader.list_columns(table_id, snapshot).await.unwrap();
    assert_eq!(columns.len(), 3);
}
```

### Wire Corpus Tests

Located in `tests/golden/`. Verify that SQL classification matches expected results for real DuckDB output:

```rust
#[test]
fn test_corpus_create_schema() {
    let sql = include_str!("../fixtures/wire-corpus/duckdb-1.2.0/create-schema.sql");
    let result = classify_statement(sql).unwrap();
    assert_eq!(result.kind, StatementKind::CreateSchema);
}
```

## Running Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p slateduck-core

# Specific test
cargo test test_schema_key_roundtrip

# With output (for debugging)
cargo test -- --nocapture
```

## Writing Good Tests

- Test the public interface, not private implementation details
- Use descriptive test names that explain what is being verified
- Keep tests focused: one assertion per logical concept
- Use helper functions to reduce test boilerplate without hiding important details
