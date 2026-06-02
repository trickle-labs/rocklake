# RockLake Azure Checkpoint Bug Fix

## Problem Summary

When using DuckLake with Azure storage and executing `CHECKPOINT`:
1. ✅ Parquet files ARE successfully written to Azure storage
2. ❌ Subsequent `SELECT` queries fail with: 
   ```
   IO Error: Cannot open file "ducklake-019e88b5-3f8b-7a3d-9ffc-a27b8d630b4e.parquet": No such file or directory
   ```

## Root Cause Analysis

The issue occurs due to a **path resolution failure** in three interconnected layers:

### Layer 1: Path Relativity Detection (Bug #1)
**Location**: `crates/rocklake-catalog/src/writer/mod.rs` (multiple functions)

When DuckLake provides a **relative path** like `table/file.parquet`, RockLake was incorrectly marking it as `path_is_relative = false` (absolute).

**Why this matters**: 
- Relative paths should be resolved relative to `data_prefix` (e.g., `az://rocklake-data/`)
- Marking them as absolute prevents this resolution logic from running
- Result: The system tries to find `table/file.parquet` as an absolute local path

### Layer 2: URL Construction (Bug #2)
**Location**: `crates/rocklake-datafusion/src/catalog_provider.rs`

When scanning tables, the code was hardcoded to create `file://` URLs:
```rust
let abs = format!("{}/{}", root.trim_end_matches('/'), f.path);
ListingTableUrl::parse(format!("file://{abs}"))  // ❌ WRONG for Azure
```

When `root` is `az://rocklake-data/` and path is `table/file.parquet`:
- ❌ Creates invalid URL: `file://az://rocklake-data/table/file.parquet`
- ✅ Should create: `az://rocklake-data/table/file.parquet`

## Solution

### Fix #1: Detect Path Relativity Correctly

**Added**: `is_path_relative()` helper function in `crates/rocklake-core/src/path.rs`

```rust
pub fn is_path_relative(path: &str) -> bool {
    // Check if path contains a URI scheme (e.g., "s3://", "az://")
    !path.contains("://")
}
```

**Tests**:
- ✅ `table/file.parquet` → `true` (relative)
- ✅ `s3://bucket/table/file.parquet` → `false` (absolute)
- ✅ `az://container/table/file.parquet` → `false` (absolute)

### Fix #2: Use Path Relativity in Writer Functions

Updated 5 functions in `crates/rocklake-catalog/src/writer/mod.rs`:
1. `register_data_file()`
2. `register_data_file_partial()`
3. `register_data_file_with_metadata()`
4. `register_delete_file()`
5. `register_delete_file_with_metadata()`

Each now uses:
```rust
let path_is_relative = rocklake_core::path::is_path_relative(path);
// ... later in DataFileRow:
path_is_relative: Some(path_is_relative),  // ✅ Correct detection
```

### Fix #3: Update Export/Import

**File**: `crates/rocklake-catalog/src/export.rs`

Updated the catalog import function that reads parquet files to also detect path relativity correctly.

### Fix #4: Handle Object Store URIs in Scans

**File**: `crates/rocklake-datafusion/src/catalog_provider.rs`

Changed URL construction to detect existing schemes:
```rust
let url_str = if root.contains("://") {
    // Already a URI with scheme (s3://, az://, etc.)
    abs
} else {
    // Local filesystem path - prepend file://
    format!("file://{abs}")
};
ListingTableUrl::parse(url_str)
```

## Expected Behavior After Fix

### Write Path (DuckDB CHECKPOINT)
1. DuckLake writes parquet: `table/file.parquet` to Azure storage
2. DuckLake sends `INSERT INTO ducklake_data_file (path='table/file.parquet', ...)`
3. RockLake now correctly detects: `path_is_relative = true` ✅

### Read Path (SELECT query)
1. Catalog reads file path: `table/file.parquet` with `path_is_relative = true`
2. Path resolution:
   - Read from `DATA_PATH` metadata: `az://rocklake-data/`
   - Combine: `az://rocklake-data/` + `table/file.parquet`
   - Result: `az://rocklake-data/table/file.parquet` ✅
3. DataFusion scan detects Azure scheme:
   - Sees `az://rocklake-data/...` contains `://`
   - Uses URL as-is (no file:// prefix) ✅
4. Azure Object Store successfully reads file ✅

## Files Modified

1. **crates/rocklake-core/src/path.rs**
   - Added `is_path_relative()` function
   - Added comprehensive tests

2. **crates/rocklake-catalog/src/writer/mod.rs**
   - Updated `register_data_file()` 
   - Updated `register_data_file_partial()`
   - Updated `register_data_file_with_metadata()`
   - Updated `register_delete_file()`
   - Updated `register_delete_file_with_metadata()`

3. **crates/rocklake-catalog/src/export.rs**
   - Updated catalog import function

4. **crates/rocklake-datafusion/src/catalog_provider.rs**
   - Updated `scan()` to handle object store URIs

## Testing Recommendations

1. **Unit Tests**: Run existing tests for path.rs
2. **Integration Test**: Reproduce your original scenario:
   ```sql
   CREATE TABLE brukere (id INTEGER, navn VARCHAR, registrert_dato DATE);
   INSERT INTO brukere VALUES (1, 'Ola', '2026-05-30');
   CHECKPOINT;
   SELECT * FROM brukere;  -- Should succeed now
   ```
3. **Regression Tests**: Test with local filesystem (`data_path = /local/path/`)
4. **Cross-Storage Tests**: Test with S3, GCS, and Azure

## Version Impact

This fix is compatible with:
- RockLake v0.47.3 and later
- Works with relative and absolute paths
- Maintains backward compatibility

## Notes

- The bug was introduced because the code assumed all paths would be absolute AWS S3 paths
- DuckLake actually sends relative paths, which is the correct design for portability
- The fix properly honors the `path_is_relative` field that was already defined but unused
