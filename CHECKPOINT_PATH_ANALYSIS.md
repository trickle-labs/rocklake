# DuckLake Parquet File Path Registration After CHECKPOINT

## Overview
This document traces how DuckLake parquet file paths are registered after CHECKPOINT and stored in the RockLake catalog, specifically examining whether paths include Azure storage prefixes.

---

## 1. Data File Registration Function

### Location
[crates/rocklake-catalog/src/writer/mod.rs#L703-L741](crates/rocklake-catalog/src/writer/mod.rs#L703-L741)

### Function Signature
```rust
pub async fn register_data_file(
    &mut self,
    table_id: u64,
    path: &str,
    file_format: &str,
    record_count: u64,
    file_size_bytes: u64,
) -> CatalogResult<u64>
```

### Key Code Snippet
```rust
let row = DataFileRow {
    data_file_id,
    table_id,
    path: path.to_string(),           // ← Stored as-is, no modification
    file_format: file_format.to_string(),
    record_count,
    file_size_bytes,
    footer_size: None,
    encryption_key: None,
    begin_snapshot: Some(snapshot_id),
    end_snapshot: None,
    file_order: Some(file_order),
    path_is_relative: Some(false),    // ← Marked as absolute path
    row_id_start: Some(row_id_start),
    partition_id: None,
    mapping_id: None,
    partial_max: None,
};

let key = keys::key_data_file(table_id, data_file_id);
let encoded = values::encode_value(&row);
// Also write the secondary index entry for O(log N) snapshot-bounded scans.
let idx_key = keys::key_data_file_by_snapshot(table_id, snapshot_id, data_file_id);
self.stage(key, encoded.clone());
self.stage(idx_key, encoded);
```

**Key Finding**: The path is stored **as-is without any modification** — RockLake does not strip or add Azure prefixes.

---

## 2. DataFileRow Structure

### Location
[crates/rocklake-core/src/rows.rs#L240-L274](crates/rocklake-core/src/rows.rs#L240-L274)

```rust
#[derive(Clone, PartialEq, prost::Message)]
pub struct DataFileRow {
    #[prost(uint64, tag = "1")]
    pub data_file_id: u64,
    #[prost(uint64, tag = "2")]
    pub table_id: u64,
    #[prost(string, tag = "3")]
    pub path: String,
    #[prost(string, tag = "4")]
    pub file_format: String,
    /// v0.24: renamed from row_count — spec field is record_count.
    #[prost(uint64, tag = "5")]
    pub record_count: u64,
    #[prost(uint64, tag = "6")]
    pub file_size_bytes: u64,
    #[prost(int64, optional, tag = "8")]
    pub footer_size: Option<i64>,
    #[prost(string, optional, tag = "9")]
    pub encryption_key: Option<String>,
    #[prost(uint64, optional, tag = "10")]
    pub begin_snapshot: Option<u64>,
    #[prost(uint64, optional, tag = "11")]
    pub end_snapshot: Option<u64>,
    #[prost(uint64, optional, tag = "12")]
    pub file_order: Option<u64>,
    // ... other fields
}
```

---

## 3. Catalog Export (NDJSON Export Format)

### Location
[crates/rocklake-catalog/src/export.rs#L220-L254](crates/rocklake-catalog/src/export.rs#L220-L254)

### Export Code
```rust
// Export data files
tables_exported += 1;
let prefix = keys::prefix_for_tag(TAG_DATA_FILE);
let mut iter = db.scan_prefix(&prefix).await?;
while let Some(kv) = iter.next().await... {
    let row: DataFileRow = values::decode_value(&kv.value)?;
    let begin = row.begin_snapshot.unwrap_or(0);
    let live_at_snapshot = begin <= dl_snapshot_id.as_u64()
        && row.end_snapshot.is_none_or(|end| end > dl_snapshot_id.as_u64());
    if live_at_snapshot {
        let exported = ExportedRow {
            table: "ducklake_data_file".to_string(),
            data: serde_json::json!({
                "data_file_id": row.data_file_id,
                "table_id": row.table_id,
                "path": row.path,                    // ← Exported as-is
                "file_format": row.file_format,
                "record_count": row.record_count,
                "file_size_bytes": row.file_size_bytes,
                "begin_snapshot": begin,
                "end_snapshot": row.end_snapshot,
                "footer_size": row.footer_size,
            }),
        };
        serde_json::to_writer(&mut *writer, &exported)?;
        writeln!(writer)?;
        rows_exported += 1;
    }
}
```

---

## 4. SQL INSERT Statement Generation

### Location
[crates/rocklake-catalog/src/export.rs#L1680](crates/rocklake-catalog/src/export.rs#L1680)

### Generated SQL Template
```rust
"INSERT INTO ducklake_data_file (data_file_id, table_id, path, file_format, row_count, file_size_bytes, snapshot_id) VALUES ({}, {}, '{}', '{}', {}, {}, {});"
```

### Example Output
```sql
INSERT INTO ducklake_data_file (data_file_id, table_id, path, file_format, row_count, file_size_bytes, snapshot_id) VALUES (1001, 42, 'az://rocklake-data/table_42/checkpoint-00001.parquet', 'parquet', 100000, 5242880, 23);
```

---

## 5. SQL Statement Parsing & Path Extraction

### Location
[crates/rocklake-pgwire/src/executor/mod.rs#L1466-L1490](crates/rocklake-pgwire/src/executor/mod.rs#L1466-L1490)

### Code for InsertDataFile Statement Kind
```rust
StatementKind::InsertDataFile => {
    let literals = literal_insert_values(_sql);
    let op = BufferedOp::InsertDataFile {
        table_id: params
            .get_u64(0)
            .ok()
            .or_else(|| literal_u64(&literals, 1))
            .unwrap_or(0),
        path: params
            .get_string(1)
            .ok()
            .or_else(|| literal_string(&literals, 5))  // ← Path extracted from literal position 5
            .unwrap_or_default(),
        file_format: params
            .get_string(2)
            .ok()
            .or_else(|| literal_string(&literals, 7))
            .unwrap_or_else(|| "parquet".to_string()),
        row_count: params
            .get_u64(3)
            .ok()
            .or_else(|| literal_u64(&literals, 8))
            .unwrap_or(0),
        file_size_bytes: params
            .get_u64(4)
            .ok()
            .or_else(|| literal_u64(&literals, 9))
            .unwrap_or(0),
    };
```

### Helper Function
```rust
fn literal_string(values: &[Option<String>], index: usize) -> Option<String> {
    values.get(index).cloned().flatten()
}
```

**Key Finding**: Path is extracted directly from SQL literals with **NO transformations** applied.

---

## 6. Path Registration in Executor

### Location
[crates/rocklake-pgwire/src/executor/catalog.rs#L182-L194](crates/rocklake-pgwire/src/executor/catalog.rs#L182-L194)

### Code
```rust
BufferedOp::InsertDataFile {
    table_id,
    path,
    file_format,
    row_count,
    file_size_bytes,
} => {
    needs_snapshot = true;
    writer
        .register_data_file(table_id, &path, &file_format, row_count, file_size_bytes)
        .await
        .map_err(RockLakeError::from)?;
}
```

**Key Finding**: The extracted path is passed directly to `register_data_file()` **without any modification**.

---

## 7. Catalog Export (ducklake_data_file Table Insertion)

### Location
[crates/rocklake-catalog/src/export.rs#L1807-1820](crates/rocklake-catalog/src/export.rs#L1807-1820)

### Import Code
```rust
"ducklake_data_file" => {
    let data_file_id = req_u64!(d, "data_file_id", tbl);
    let table_id = req_u64!(d, "table_id", tbl);
    let begin_snapshot = d["begin_snapshot"]
        .as_u64()
        .or_else(|| d["snapshot_id"].as_u64());
    let row = DataFileRow {
        data_file_id,
        table_id,
        path: req_str!(d, "path", tbl),  // ← Path extracted from JSON
        file_format: req_str!(d, "file_format", tbl),
        record_count: d["record_count"]
            .as_u64()
            .or_else(|| d["row_count"].as_u64())
            .unwrap_or(0),
        file_size_bytes: req_u64!(d, "file_size_bytes", tbl),
        footer_size: d["footer_size"].as_i64(),
        encryption_key: d["encryption_key"].as_str().map(|s| s.to_string()),
        begin_snapshot,
        end_snapshot: d["end_snapshot"].as_u64(),
        file_order: d["file_order"].as_u64(),
        path_is_relative: d["path_is_relative"].as_bool(),
        // ... other fields
    };
    let encoded = values::encode_value(&row);
    // Write primary and secondary index atomically
    let mut batch = WriteBatch::new();
    batch.put(keys::key_data_file(table_id, data_file_id), encoded.clone());
    let idx_begin = begin_snapshot.unwrap_or(0);
    batch.put(
        keys::key_data_file_by_snapshot(table_id, idx_begin, data_file_id),
        encoded,
    );
    db.write(batch).await?;
    rows_imported += 1;
}
```

---

## 8. Path Usage During Reads (DataFusion Provider)

### Location
[crates/rocklake-datafusion/src/catalog_provider.rs#L400-420](crates/rocklake-datafusion/src/catalog_provider.rs#L400-420)

### Code
```rust
let desc = reader
    .describe_table(table.table_id)
    .await
    .map_err(|e| DataFusionError::External(Box::new(e)))?;

match desc {
    None => Ok(None),
    Some((_table_row, columns)) => {
        // Propagate catalog errors rather than silently returning empty results.
        let data_files = reader
            .list_data_files(table.table_id)
            .await
            .map_err(|e| DataFusionError::External(Box::new(e)))?;

        let table_provider = RockLakeTableProvider::new(
            table.table_name.clone(),
            table.table_id,
            columns,
            data_files,
            self.data_root.clone(),  // ← data_root from catalog metadata
        )?;
        Ok(Some(Arc::new(table_provider)))
    }
}
```

---

## 9. Path Resolution Mode

### Location
[crates/rocklake-core/src/path.rs#L47-L67](crates/rocklake-core/src/path.rs#L47-L67)

```rust
pub fn resolve_data_path(&self, stored_path: &str) -> String {
    match self.data_path_mode {
        DataPathMode::Absolute => stored_path.to_string(),  // ← Uses path as-is
        DataPathMode::RelativeToDataPrefix => {
            format!(
                "{}{}",
                self.data_prefix,
                stored_path.trim_start_matches('/')
            )
        }
    }
}
```

---

## Key Findings Summary

### 1. **Path Storage**
- Paths are stored **exactly as provided by DuckLake** in the SQL INSERT statement
- RockLake performs **NO transformation** of the path string
- The `path_is_relative` flag is set to `false`, marking the path as absolute

### 2. **Azure Prefixes**
- **If DuckLake provides the full Azure URL** (e.g., `az://rocklake-data/table/file.parquet`):
  - ✅ It's stored with the full prefix
  - ✅ Reads will correctly use the full path

- **If DuckLake provides only the relative path** (e.g., `table/file.parquet`):
  - ⚠️ It's stored as-is with `path_is_relative: false`
  - ⚠️ Reads will treat it as an absolute path and may fail to resolve it correctly

### 3. **Path Extraction Points** (No modifications occur at these points)
1. **SQL Parse**: `literal_insert_values()` → extracts from SQL string position 5
2. **Statement Kind**: `StatementKind::InsertDataFile` classification
3. **Parameter Extraction**: `literal_string()` clones value as-is
4. **BufferedOp Creation**: Path stored directly in operation
5. **Executor**: Passed to `writer.register_data_file()`
6. **Writer**: Stored in `DataFileRow` as `path: path.to_string()`

### 4. **Export Format**
- Exported NDJSON contains the exact same path string stored in the catalog
- PostgreSQL migration uses the path verbatim in INSERT statements

### 5. **Path Resolution During Reads**
- Two modes available:
  - **Absolute**: Path used as-is
  - **RelativeToDataPrefix**: `data_prefix` prepended to path
- Current default: **Absolute mode** (paths treated as complete URIs)

---

## Testing Example

### Scenario: DuckLake CHECKPOINT on Azure
```sql
-- DuckLake sends INSERT (exact SQL):
INSERT INTO ducklake_data_file (data_file_id, table_id, path, file_format, row_count, file_size_bytes, snapshot_id) 
VALUES (1001, 42, 'az://rocklake-data/table_42/checkpoint-00001.parquet', 'parquet', 100000, 5242880, 23);

-- RockLake receives and registers:
register_data_file(
    table_id=42,
    path="az://rocklake-data/table_42/checkpoint-00001.parquet",  // ← Stored with full Azure prefix
    file_format="parquet",
    record_count=100000,
    file_size_bytes=5242880
)

-- Stored in DataFileRow:
{
    data_file_id: 1001,
    table_id: 42,
    path: "az://rocklake-data/table_42/checkpoint-00001.parquet",
    path_is_relative: false,  // ← Marked as absolute
    // ...
}

-- On read via DataFusion:
resolve_data_path("az://rocklake-data/table_42/checkpoint-00001.parquet")
// Returns: "az://rocklake-data/table_42/checkpoint-00001.parquet" (unchanged because Absolute mode)
```

---

## Potential Issues

### Issue 1: Relative vs. Absolute Mode Mismatch
If `path_is_relative` is set to `true` but mode is still `Absolute`, reads will fail because:
- Path stored: `table_42/checkpoint-00001.parquet`
- Resolve in Absolute mode: returns path unchanged → file not found

### Issue 2: Missing Azure Prefix
If DuckLake only provides relative path but system is in Absolute mode:
- Path stored: `table_42/checkpoint-00001.parquet`
- Expected: `az://rocklake-data/table_42/checkpoint-00001.parquet`
- Actual: `table_42/checkpoint-00001.parquet` (relative path treated as literal)

### Issue 3: Path Flag Bug (v0.24+ still present)
`path_is_relative` is **always set to `false`** in `register_data_file()`:
```rust
path_is_relative: Some(false),  // ← Always false, regardless of actual path content
```

This means RockLake cannot distinguish between absolute and relative paths during reads.

---

## Recommendations

1. **Verify DuckLake Output**: Confirm whether DuckLake sends:
   - Full Azure URLs: `az://rocklake-data/...` ✅
   - Or relative paths: `table/...` ⚠️

2. **Check Data Path Mode**: Ensure system is configured for the correct mode:
   - Absolute: For full URI paths
   - Relative: For paths relative to `data_prefix`

3. **Fix `path_is_relative` Flag**: Consider:
   - Detect whether path contains `://` (scheme indicator)
   - Set flag accordingly during registration
   - Update import to respect the flag

4. **Test with Azure Emulator**: Run tests with `--features azure-emulator` to validate full Azure path handling
