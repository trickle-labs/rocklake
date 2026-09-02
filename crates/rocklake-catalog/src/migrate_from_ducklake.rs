//! Migration from an external DuckLake catalog into RockLake.
//!
//! Reads a PostgreSQL- or SQLite-backed DuckLake catalog, replays its current
//! snapshot into a fresh RockLake catalog using the standard write API, and
//! emits a verification report.  Data files are **not** copied — they remain at
//! their original object-store paths.
//!
//! # Version gating
//!
//! Only DuckLake catalog version 7 (V1_0) is supported by default.  If the
//! source catalog reports version 8 (V1_1_DEV_1), the migration is rejected
//! with `CatalogError::UnsupportedDuckLakeVersion` (SQLSTATE 0A000) unless the
//! caller passes `ACCEPT_VERSION_V1_1_DEV_1` in `accept_versions`.
//!
//! # Architecture
//!
//! ```text
//! PostgreSQL/SQLite/NDJSON source
//!         ↓  DuckLakeSource trait
//!   collect ExportedRows (NDJSON-compatible)
//!         ↓
//!   migrate_from_source()
//!         ↓  WriteBatch (atomic primary + secondary keys)
//!   RockLake SlateDB catalog
//! ```

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::BufReader;

use base64::Engine as _;
use slatedb::{Db, WriteBatch};

use rocklake_core::rows::*;
use rocklake_core::{keys, values};
use serde_json::Value;

use crate::error::{CatalogError, CatalogResult};
use crate::export::ExportedRow;

// ── Version constants ────────────────────────────────────────────────────────

/// DuckLake catalog schema version 7 = V1_0 (the only supported version).
pub const DUCKLAKE_V1_0_CATALOG_VERSION: u64 = 7;
/// DuckLake catalog schema version 8 = V1_1_DEV_1 (forward-compat pre-release).
pub const DUCKLAKE_V1_1_DEV_1_CATALOG_VERSION: u64 = 8;
/// The `--accept-version` token for DuckLake v1.1 pre-release.
pub const ACCEPT_VERSION_V1_1_DEV_1: &str = "V1_1_DEV_1";

// ── Report types ─────────────────────────────────────────────────────────────

/// Per-table statistics inside a [`MigrationReport`].
#[derive(Debug, Clone, Default)]
pub struct TableStats {
    /// Rows successfully written to the target catalog.
    pub rows_migrated: u64,
    /// Rows that could not be decoded and were skipped (with a warning).
    pub rows_skipped: u64,
}

/// Verification report returned by a migration run or dry-run.
#[derive(Debug, Clone)]
pub struct MigrationReport {
    /// Per-table statistics (keyed by DuckLake table name).
    pub tables: HashMap<String, TableStats>,
    /// Snapshot ID range observed in the source (min, max).
    pub snapshot_id_range: Option<(u64, u64)>,
    /// Number of data files migrated.
    pub data_file_count: u64,
    /// Whether this was a dry-run (no writes performed).
    pub dry_run: bool,
    /// Source catalog DuckLake catalog_version (schema_version of the latest snapshot).
    pub source_catalog_version: u64,
    /// Exact source snapshot used for the migration.
    pub source_snapshot_id: u64,
}

impl MigrationReport {
    /// Total rows migrated across all tables.
    pub fn total_migrated(&self) -> u64 {
        self.tables.values().map(|t| t.rows_migrated).sum()
    }
    /// Total rows skipped across all tables.
    pub fn total_skipped(&self) -> u64 {
        self.tables.values().map(|t| t.rows_skipped).sum()
    }
}

// ── Source abstraction ────────────────────────────────────────────────────────

/// Abstraction over a DuckLake source catalog.
///
/// Implementations exist for:
/// - [`InMemoryDuckLakeSource`] — for testing
/// - [`SqliteDuckLakeSource`] — reads from a SQLite DuckLake catalog via `rusqlite`
/// - [`PostgresDuckLakeSource`] — reads from a PostgreSQL DuckLake catalog
pub trait DuckLakeSource {
    /// Return the DuckLake catalog schema version (e.g., 7 = V1_0, 8 = V1_1_DEV_1).
    ///
    /// Derived from `MAX(schema_version)` across all snapshot rows.
    fn catalog_version(&mut self) -> CatalogResult<u64>;

    /// Read all rows from the named DuckLake table and return them as
    /// `ExportedRow` values.  The implementation applies the correct MVCC
    /// predicate (`begin_snapshot <= N AND (end_snapshot IS NULL OR
    /// end_snapshot > N)`) before returning rows.
    fn read_table(&mut self, table: &str) -> CatalogResult<Vec<ExportedRow>>;
}

// ── InMemoryDuckLakeSource ────────────────────────────────────────────────────

/// An in-memory DuckLake source for testing and for use as the intermediate
/// representation when reading from a live PostgreSQL catalog.
#[derive(Default)]
pub struct InMemoryDuckLakeSource {
    version: u64,
    tables: HashMap<String, Vec<ExportedRow>>,
}

impl InMemoryDuckLakeSource {
    /// Create an empty in-memory source with the given `catalog_version`.
    pub fn new(version: u64) -> Self {
        Self {
            version,
            tables: Default::default(),
        }
    }

    /// Seed rows for a table.
    pub fn add_rows(&mut self, table: &str, rows: Vec<ExportedRow>) {
        self.tables
            .entry(table.to_string())
            .or_default()
            .extend(rows);
    }
}

impl DuckLakeSource for InMemoryDuckLakeSource {
    fn catalog_version(&mut self) -> CatalogResult<u64> {
        Ok(self.version)
    }

    fn read_table(&mut self, table: &str) -> CatalogResult<Vec<ExportedRow>> {
        Ok(self.tables.get(table).cloned().unwrap_or_default())
    }
}

// ── SqliteDuckLakeSource ──────────────────────────────────────────────────────

/// A DuckLake source that reads from a SQLite database using `rusqlite`.
///
/// Queries standard DuckLake tables with a correct MVCC predicate so only
/// rows visible at the current (latest) snapshot are returned.
pub struct SqliteDuckLakeSource {
    /// The snapshot ID at which rows were read.
    pub snapshot_id: u64,
    /// All rows collected from the SQLite database, keyed by table name.
    rows: HashMap<String, Vec<ExportedRow>>,
    /// The catalog version read from `MAX(schema_version)` in snapshots.
    version: u64,
}

#[allow(dead_code)]
impl SqliteDuckLakeSource {
    /// Open a SQLite DuckLake catalog at `path`.
    ///
    /// Pass `snapshot_id = None` to read at the latest snapshot.
    pub fn open(path: &str, snapshot_id: Option<u64>) -> CatalogResult<Self> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| CatalogError::MigrationSource(format!("SQLite open '{path}': {e}")))?;
        Self::from_connection(conn, snapshot_id)
    }

    /// Create from an already-opened `rusqlite::Connection` (useful for
    /// in-memory databases in tests: `Connection::open_in_memory()`).
    pub fn from_connection(
        conn: rusqlite::Connection,
        snapshot_id: Option<u64>,
    ) -> CatalogResult<Self> {
        let sid: u64 = match snapshot_id {
            Some(id) => id,
            None => conn
                .query_row(
                    "SELECT MAX(snapshot_id) FROM ducklake_snapshot",
                    [],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
                .ok_or_else(|| {
                    CatalogError::MigrationSource(
                        "DuckLake source has no snapshot rows".to_string(),
                    )
                })?
                .try_into()
                .map_err(|_| {
                    CatalogError::MigrationSource(
                        "DuckLake snapshot_id must be a non-negative integer".to_string(),
                    )
                })?,
        };

        let version: u64 = conn
            .query_row(
                "SELECT MAX(schema_version) FROM ducklake_snapshot",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .ok_or_else(|| {
                CatalogError::MigrationSource("DuckLake source has no schema version".to_string())
            })?
            .try_into()
            .map_err(|_| {
                CatalogError::MigrationSource(
                    "DuckLake schema_version must be a non-negative integer".to_string(),
                )
            })?;

        let mut src = Self {
            snapshot_id: sid,
            rows: Default::default(),
            version,
        };

        src.load_all_tables(&conn, sid)?;
        Ok(src)
    }

    fn load_all_tables(&mut self, conn: &rusqlite::Connection, sid: u64) -> CatalogResult<()> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let source_tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .collect::<Result<_, _>>()
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        for table in source_tables {
            if table.starts_with("sqlite_") {
                continue;
            }
            if source_target_table(&table).is_none() {
                return Err(CatalogError::MigrationSource(format!(
                    "unsupported DuckLake source table '{table}'"
                )));
            }
        }
        for &target in DUCKLAKE_TABLES {
            let source = source_table_for_target(target);
            if Self::table_exists(conn, source) {
                self.load_table(conn, sid, source, target)?;
            }
        }
        Ok(())
    }

    fn load_table(
        &mut self,
        conn: &rusqlite::Connection,
        sid: u64,
        source: &str,
        target: &str,
    ) -> CatalogResult<()> {
        let pragma = format!("PRAGMA table_info({source})");
        let mut columns_stmt = conn
            .prepare(&pragma)
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let columns: Vec<String> = columns_stmt
            .query_map([], |row| row.get(1))
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .collect::<Result<_, _>>()
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let where_clause = if columns.iter().any(|c| c == "begin_snapshot") {
            " WHERE (begin_snapshot IS NULL OR begin_snapshot <= ?1) \
              AND (end_snapshot IS NULL OR end_snapshot > ?1)"
        } else if columns.iter().any(|c| c == "snapshot_id") {
            " WHERE snapshot_id <= ?1"
        } else {
            ""
        };
        let sql = format!("SELECT * FROM {source}{where_clause}");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        let params: &[&dyn rusqlite::ToSql] = if where_clause.is_empty() {
            &[]
        } else {
            &[&(sid as i64)]
        };
        let rows = stmt
            .query_map(params, |row| {
                let mut object = serde_json::Map::new();
                for (index, name) in names.iter().enumerate() {
                    let value: rusqlite::types::Value = row.get(index)?;
                    object.insert(name.clone(), rusqlite_value_to_json(value));
                }
                Ok(ExportedRow {
                    table: target.to_string(),
                    data: Value::Object(object),
                })
            })
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        self.rows.insert(target.to_string(), rows);
        Ok(())
    }

    fn load_snapshots(&mut self, conn: &rusqlite::Connection, sid: u64) -> CatalogResult<()> {
        let mut stmt = conn
            .prepare(
                "SELECT snapshot_id, schema_version, snapshot_time, author, message \
                 FROM ducklake_snapshot WHERE snapshot_id <= ?1",
            )
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;

        let rows: Vec<ExportedRow> = stmt
            .query_map([sid as i64], |row| {
                Ok(serde_json::json!({
                    "snapshot_id": row.get::<_, i64>(0).unwrap_or(0) as u64,
                    "schema_version": row.get::<_, i64>(1).unwrap_or(0) as u64,
                    "snapshot_time": row.get::<_, Option<String>>(2).unwrap_or(None).unwrap_or_default(),
                    "author": row.get::<_, Option<String>>(3).unwrap_or(None),
                    "message": row.get::<_, Option<String>>(4).unwrap_or(None),
                }))
            })
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .filter_map(|r| r.ok())
            .map(|data| ExportedRow { table: "ducklake_snapshot".to_string(), data })
            .collect();

        self.rows
            .entry("ducklake_snapshot".to_string())
            .or_default()
            .extend(rows);
        Ok(())
    }

    fn load_snapshot_changes(
        &mut self,
        conn: &rusqlite::Connection,
        sid: u64,
    ) -> CatalogResult<()> {
        let mut stmt = conn
            .prepare(
                "SELECT snapshot_id, change_type, change_info, schema_id, table_id, \
                 author, commit_message \
                 FROM ducklake_snapshot_changes WHERE snapshot_id <= ?1",
            )
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;

        let rows: Vec<ExportedRow> = stmt
            .query_map([sid as i64], |row| {
                Ok(serde_json::json!({
                    "snapshot_id": row.get::<_, i64>(0).unwrap_or(0) as u64,
                    "change_type": row.get::<_, Option<String>>(1).unwrap_or(None).unwrap_or_default(),
                    "change_info": row.get::<_, Option<String>>(2).unwrap_or(None),
                    "schema_id": row.get::<_, Option<i64>>(3).unwrap_or(None).map(|v| v as u64),
                    "table_id": row.get::<_, Option<i64>>(4).unwrap_or(None).map(|v| v as u64),
                    "author": row.get::<_, Option<String>>(5).unwrap_or(None),
                    "commit_message": row.get::<_, Option<String>>(6).unwrap_or(None),
                }))
            })
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .filter_map(|r| r.ok())
            .map(|data| ExportedRow { table: "ducklake_snapshot_changes".to_string(), data })
            .collect();

        self.rows
            .entry("ducklake_snapshot_changes".to_string())
            .or_default()
            .extend(rows);
        Ok(())
    }

    fn load_data_files(&mut self, conn: &rusqlite::Connection, sid: u64) -> CatalogResult<()> {
        let mut stmt = conn
            .prepare(
                "SELECT data_file_id, table_id, path, file_format, record_count, \
                 file_size_bytes, begin_snapshot, end_snapshot, footer_size \
                 FROM ducklake_data_file \
                 WHERE (begin_snapshot IS NULL OR begin_snapshot <= ?1) \
                 AND (end_snapshot IS NULL OR end_snapshot > ?1)",
            )
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;

        let rows: Vec<ExportedRow> = stmt
            .query_map([sid as i64], |row| {
                Ok(serde_json::json!({
                    "data_file_id": row.get::<_, i64>(0).unwrap_or(0) as u64,
                    "table_id": row.get::<_, i64>(1).unwrap_or(0) as u64,
                    "path": row.get::<_, String>(2).unwrap_or_default(),
                    "file_format": row.get::<_, Option<String>>(3).unwrap_or(None).unwrap_or_else(|| "parquet".to_string()),
                    "record_count": row.get::<_, Option<i64>>(4).unwrap_or(None).unwrap_or(0) as u64,
                    "file_size_bytes": row.get::<_, Option<i64>>(5).unwrap_or(None).unwrap_or(0) as u64,
                    "begin_snapshot": row.get::<_, Option<i64>>(6).unwrap_or(None).map(|v| v as u64),
                    "end_snapshot": row.get::<_, Option<i64>>(7).unwrap_or(None).map(|v| v as u64),
                    "footer_size": row.get::<_, Option<i64>>(8).unwrap_or(None),
                }))
            })
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .filter_map(|r| r.ok())
            .map(|data| ExportedRow { table: "ducklake_data_file".to_string(), data })
            .collect();

        self.rows
            .entry("ducklake_data_file".to_string())
            .or_default()
            .extend(rows);
        Ok(())
    }

    fn load_delete_files(&mut self, conn: &rusqlite::Connection, sid: u64) -> CatalogResult<()> {
        let mut stmt = conn
            .prepare(
                "SELECT delete_file_id, data_file_id, path, delete_count, file_size_bytes, \
                 snapshot_id, begin_snapshot, end_snapshot, encryption_key \
                 FROM ducklake_delete_file \
                 WHERE (begin_snapshot IS NULL OR begin_snapshot <= ?1) \
                 AND (end_snapshot IS NULL OR end_snapshot > ?1)",
            )
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;

        let rows: Vec<ExportedRow> = stmt
            .query_map([sid as i64], |row| {
                Ok(serde_json::json!({
                    "delete_file_id": row.get::<_, i64>(0).unwrap_or(0) as u64,
                    "data_file_id": row.get::<_, i64>(1).unwrap_or(0) as u64,
                    "path": row.get::<_, String>(2).unwrap_or_default(),
                    "delete_count": row.get::<_, Option<i64>>(3).unwrap_or(None).unwrap_or(0) as u64,
                    "file_size_bytes": row.get::<_, Option<i64>>(4).unwrap_or(None).unwrap_or(0) as u64,
                    "snapshot_id": row.get::<_, Option<i64>>(5).unwrap_or(None).unwrap_or(0) as u64,
                    "begin_snapshot": row.get::<_, Option<i64>>(6).unwrap_or(None).map(|v| v as u64),
                    "end_snapshot": row.get::<_, Option<i64>>(7).unwrap_or(None).map(|v| v as u64),
                    "encryption_key": row.get::<_, Option<String>>(8).unwrap_or(None),
                }))
            })
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .filter_map(|r| r.ok())
            .map(|data| ExportedRow { table: "ducklake_delete_file".to_string(), data })
            .collect();

        self.rows
            .entry("ducklake_delete_file".to_string())
            .or_default()
            .extend(rows);
        Ok(())
    }

    fn load_versioned_generic(
        &mut self,
        conn: &rusqlite::Connection,
        sid: u64,
        table: &str,
    ) -> CatalogResult<()> {
        let sql = format!(
            "SELECT * FROM {table} WHERE begin_snapshot <= ?1 \
             AND (end_snapshot IS NULL OR end_snapshot > ?1)"
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;

        let col_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        let rows: Vec<ExportedRow> = stmt
            .query_map([sid as i64], |row| {
                let mut map = serde_json::Map::new();
                for (i, name) in col_names.iter().enumerate() {
                    let val: rusqlite::types::Value =
                        row.get(i).unwrap_or(rusqlite::types::Value::Null);
                    map.insert(name.clone(), rusqlite_value_to_json(val));
                }
                Ok(map)
            })
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
            .filter_map(|r| r.ok())
            .map(|map| ExportedRow {
                table: table.to_string(),
                data: Value::Object(map),
            })
            .collect();

        self.rows.entry(table.to_string()).or_default().extend(rows);
        Ok(())
    }

    fn table_exists(conn: &rusqlite::Connection, table: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false)
    }
}

impl DuckLakeSource for SqliteDuckLakeSource {
    fn catalog_version(&mut self) -> CatalogResult<u64> {
        Ok(self.version)
    }

    fn read_table(&mut self, table: &str) -> CatalogResult<Vec<ExportedRow>> {
        Ok(self.rows.get(table).cloned().unwrap_or_default())
    }
}

/// A DuckLake source that snapshots a PostgreSQL catalog into memory before
/// migration. The connection stays out of the migration trait, so the target
/// write remains one atomic operation after all source reads complete.
pub struct PostgresDuckLakeSource {
    /// The snapshot ID at which rows were read.
    pub snapshot_id: u64,
    rows: HashMap<String, Vec<ExportedRow>>,
    version: u64,
}

impl PostgresDuckLakeSource {
    /// Connect to PostgreSQL and read the complete supported DuckLake catalog.
    pub async fn connect(connection_string: &str, snapshot_id: Option<u64>) -> CatalogResult<Self> {
        let (client, connection) =
            tokio_postgres::connect(connection_string, tokio_postgres::NoTls)
                .await
                .map_err(|e| CatalogError::MigrationSource(format!("PostgreSQL connect: {e}")))?;
        tokio::spawn(async move {
            if let Err(error) = connection.await {
                tracing::warn!("DuckLake PostgreSQL connection closed: {error}");
            }
        });

        let row = client
            .query_one(
                "SELECT COALESCE(MAX(snapshot_id), 0), \
                        COALESCE(MAX(schema_version), 0) \
                 FROM ducklake_snapshot",
                &[],
            )
            .await
            .map_err(|e| {
                CatalogError::MigrationSource(format!("PostgreSQL snapshot query: {e}"))
            })?;
        let latest_snapshot = row
            .try_get::<_, i64>(0)
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let version = row
            .try_get::<_, i64>(1)
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let sid = snapshot_id.unwrap_or(latest_snapshot.max(0) as u64);

        let names = client
            .query(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'",
                &[],
            )
            .await
            .map_err(|e| CatalogError::MigrationSource(format!("PostgreSQL table list: {e}")))?;
        for row in names {
            let name = row
                .try_get::<_, String>(0)
                .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
            if source_target_table(&name).is_none() {
                return Err(CatalogError::MigrationSource(format!(
                    "unsupported DuckLake source table '{name}'"
                )));
            }
        }

        let mut source = Self {
            snapshot_id: sid,
            rows: HashMap::new(),
            version: version.max(0) as u64,
        };
        for &target in DUCKLAKE_TABLES {
            let source_table = source_table_for_target(target);
            let exists = client
                .query_opt(
                    "SELECT 1 FROM information_schema.tables \
                     WHERE table_schema = current_schema() AND table_name = $1",
                    &[&source_table],
                )
                .await
                .map_err(|e| CatalogError::MigrationSource(e.to_string()))?
                .is_some();
            if exists {
                source
                    .load_table(&client, sid, source_table, target)
                    .await?;
            }
        }
        Ok(source)
    }

    async fn load_table(
        &mut self,
        client: &tokio_postgres::Client,
        sid: u64,
        source: &str,
        target: &str,
    ) -> CatalogResult<()> {
        let columns = client
            .query(
                "SELECT column_name FROM information_schema.columns \
                 WHERE table_schema = current_schema() AND table_name = $1 \
                 ORDER BY ordinal_position",
                &[&source],
            )
            .await
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let column_names: Vec<String> = columns
            .iter()
            .map(|row| row.try_get(0))
            .collect::<Result<_, _>>()
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let where_clause = if column_names.iter().any(|c| c == "begin_snapshot") {
            " WHERE (begin_snapshot IS NULL OR begin_snapshot <= $1) \
                  AND (end_snapshot IS NULL OR end_snapshot > $1)"
        } else if column_names.iter().any(|c| c == "snapshot_id") {
            " WHERE snapshot_id <= $1"
        } else {
            ""
        };
        let sql = format!(
            "SELECT COALESCE(json_agg(row_to_json(t)), '[]'::json)::text \
             FROM (SELECT * FROM {source}{where_clause}) t"
        );
        let row = if where_clause.is_empty() {
            client.query_one(&sql, &[]).await
        } else {
            client.query_one(&sql, &[&(sid as i64)]).await
        }
        .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let json = row
            .try_get::<_, String>(0)
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        let data: Vec<Value> = serde_json::from_str(&json)
            .map_err(|e| CatalogError::MigrationSource(format!("{source}: {e}")))?;
        self.rows.insert(
            target.to_string(),
            data.into_iter()
                .map(|data| ExportedRow {
                    table: target.to_string(),
                    data,
                })
                .collect(),
        );
        Ok(())
    }
}

impl DuckLakeSource for PostgresDuckLakeSource {
    fn catalog_version(&mut self) -> CatalogResult<u64> {
        Ok(self.version)
    }

    fn read_table(&mut self, table: &str) -> CatalogResult<Vec<ExportedRow>> {
        Ok(self.rows.get(table).cloned().unwrap_or_default())
    }
}

// ── Core migration logic ──────────────────────────────────────────────────────

/// All standard DuckLake spec tables (in import order).
const DUCKLAKE_TABLES: &[&str] = &[
    "ducklake_metadata",
    "ducklake_snapshot",
    "ducklake_snapshot_changes",
    "ducklake_schema",
    "ducklake_table",
    "ducklake_column",
    "ducklake_view",
    "ducklake_macro",
    "ducklake_macro_impl",
    "ducklake_macro_parameters",
    "ducklake_data_file",
    "ducklake_delete_file",
    "ducklake_files_scheduled_for_deletion",
    "ducklake_inlined_data_tables",
    "ducklake_column_mapping",
    "ducklake_name_mapping",
    "ducklake_table_stats",
    "ducklake_table_column_stats",
    "ducklake_file_column_stats",
    "ducklake_file_variant_stats",
    "ducklake_partition_info",
    "ducklake_partition_column",
    "ducklake_file_partition_value",
    "ducklake_sort_info",
    "ducklake_sort_expression",
    "ducklake_tag",
    "ducklake_column_tag",
    "ducklake_schema_version",
];

fn source_table_for_target(target: &str) -> &str {
    if target == "ducklake_schema_version" {
        "ducklake_schema_versions"
    } else {
        target
    }
}

fn source_target_table(source: &str) -> Option<&'static str> {
    if source == "ducklake_schema_versions" {
        return Some("ducklake_schema_version");
    }
    DUCKLAKE_TABLES
        .iter()
        .copied()
        .find(|target| *target == source)
}

/// Migrate from a DuckLake source into a fresh RockLake `Db`.
///
/// # Version gating
///
/// If the source catalog version is > 7 (V1_0) and the version string is not
/// included in `accept_versions`, returns
/// `CatalogError::UnsupportedDuckLakeVersion` with SQLSTATE 0A000.
///
/// # Atomicity
///
/// All source rows are validated before the importer writes one atomic batch.
/// An error leaves the target untouched.
pub async fn migrate_from_source(
    source: &mut dyn DuckLakeSource,
    db: &Db,
    accept_versions: &[&str],
    dry_run: bool,
) -> CatalogResult<MigrationReport> {
    let catalog_version = source.catalog_version()?;
    if catalog_version > DUCKLAKE_V1_0_CATALOG_VERSION {
        let token = if catalog_version == DUCKLAKE_V1_1_DEV_1_CATALOG_VERSION {
            ACCEPT_VERSION_V1_1_DEV_1
        } else {
            "UNKNOWN_VERSION"
        };
        if !accept_versions.contains(&token) {
            return Err(CatalogError::UnsupportedDuckLakeVersion {
                version: catalog_version,
                message: format!(
                    "DuckLake catalog version {catalog_version} is not supported. \
                     Use --accept-version {token} to opt into experimental support. \
                     (SQLSTATE 0A000)"
                ),
            });
        }
    }

    let mut report = MigrationReport {
        tables: HashMap::new(),
        snapshot_id_range: None,
        data_file_count: 0,
        dry_run,
        source_catalog_version: catalog_version,
        source_snapshot_id: 0,
    };

    let mut all_rows = Vec::new();
    for &tbl in DUCKLAKE_TABLES {
        let rows = source.read_table(tbl)?;
        let stats = report.tables.entry(tbl.to_string()).or_default();
        stats.rows_migrated = rows.len() as u64;
        if tbl == "ducklake_data_file" {
            report.data_file_count = rows.len() as u64;
        }
        for row in &rows {
            let mut row = row.clone();
            normalize_ducklake_row(tbl, &mut row.data);
            crate::export::validate_import_data(tbl, &row.data, 0)?;
            if tbl == "ducklake_snapshot" {
                let sid = row.data["snapshot_id"].as_u64().ok_or_else(|| {
                    CatalogError::MigrationSource("snapshot_id must be a u64".to_string())
                })?;
                report.source_snapshot_id = report.source_snapshot_id.max(sid);
                report.snapshot_id_range = Some(match report.snapshot_id_range {
                    None => (sid, sid),
                    Some((min, max)) => (min.min(sid), max.max(sid)),
                });
            }
            all_rows.push(row);
        }
    }
    if report.snapshot_id_range.is_none() {
        return Err(CatalogError::MigrationSource(
            "DuckLake source has no snapshot rows".to_string(),
        ));
    }
    if dry_run {
        return Ok(report);
    }

    let mut ndjson = Vec::new();
    for row in all_rows {
        serde_json::to_writer(&mut ndjson, &row)
            .map_err(|e| CatalogError::MigrationSource(e.to_string()))?;
        ndjson.push(b'\n');
    }
    let imported = crate::export::import_catalog(db, BufReader::new(ndjson.as_slice())).await?;
    if imported.rows_imported != report.total_migrated() {
        return Err(CatalogError::MigrationSource(format!(
            "imported {} rows but validated {} source rows",
            imported.rows_imported,
            report.total_migrated()
        )));
    }
    Ok(report)
}

/// Write a single DuckLake row into a `WriteBatch`.
///
/// Data files are written with both the primary key and the secondary
/// `TAG_DATA_FILE_BY_SNAPSHOT` index in the same batch so they are committed
/// atomically.
#[allow(dead_code)]
fn write_row_to_batch(
    batch: &mut WriteBatch,
    table: &str,
    d: &Value,
    report: &mut MigrationReport,
) -> CatalogResult<()> {
    match table {
        "ducklake_snapshot" => {
            let snapshot_id = req_u64(d, "snapshot_id", table)?;
            let row = SnapshotRow {
                snapshot_id,
                schema_version: d["schema_version"].as_u64().unwrap_or(0),
                snapshot_time: d["snapshot_time"].as_str().unwrap_or_default().to_string(),
                author: d["author"].as_str().map(|s| s.to_string()),
                message: d["message"].as_str().map(|s| s.to_string()),
                next_catalog_id: d["next_catalog_id"].as_u64(),
                next_file_id: d["next_file_id"].as_u64(),
            };
            report.snapshot_id_range = Some(match report.snapshot_id_range {
                None => (snapshot_id, snapshot_id),
                Some((min, max)) => (min.min(snapshot_id), max.max(snapshot_id)),
            });
            batch.put(keys::key_snapshot(snapshot_id), values::encode_value(&row));
        }
        "ducklake_schema" => {
            let schema_id = req_u64(d, "schema_id", table)?;
            let row = SchemaRow {
                schema_id,
                schema_name: req_str(d, "schema_name", table)?,
                begin_snapshot: d["begin_snapshot"].as_u64().unwrap_or(0),
                end_snapshot: d["end_snapshot"].as_u64(),
                schema_uuid: d["schema_uuid"].as_str().map(|s| s.to_string()),
                path: d["path"].as_str().map(|s| s.to_string()),
                path_is_relative: d["path_is_relative"].as_bool(),
            };
            batch.put(keys::key_schema(schema_id), values::encode_value(&row));
        }
        "ducklake_table" => {
            let table_id = req_u64(d, "table_id", table)?;
            let schema_id = req_u64(d, "schema_id", table)?;
            let begin_snapshot = d["begin_snapshot"].as_u64().unwrap_or(0);
            let row = TableRow {
                table_id,
                schema_id,
                table_name: req_str(d, "table_name", table)?,
                begin_snapshot,
                end_snapshot: d["end_snapshot"].as_u64(),
                path: d["path"]
                    .as_str()
                    .or_else(|| d["data_path"].as_str())
                    .map(|s| s.to_string()),
                table_uuid: d["table_uuid"].as_str().map(|s| s.to_string()),
                path_is_relative: d["path_is_relative"].as_bool(),
            };
            batch.put(
                keys::key_table(schema_id, table_id, begin_snapshot),
                values::encode_value(&row),
            );
        }
        "ducklake_column" => {
            let column_id = req_u64(d, "column_id", table)?;
            let table_id = req_u64(d, "table_id", table)?;
            let begin_snapshot = d["begin_snapshot"].as_u64().unwrap_or(0);
            let row = ColumnRow {
                column_id,
                table_id,
                column_name: req_str(d, "column_name", table)?,
                data_type: d["data_type"]
                    .as_str()
                    .or_else(|| d["column_type"].as_str())
                    .ok_or_else(|| CatalogError::Import {
                        line: 0,
                        table: table.to_string(),
                        message: "missing data_type".to_string(),
                    })?
                    .to_string(),
                column_index: d["column_index"]
                    .as_u64()
                    .or_else(|| d["column_order"].as_u64())
                    .unwrap_or(0),
                begin_snapshot,
                end_snapshot: d["end_snapshot"].as_u64(),
                default_value: d["default_value"].as_str().map(|s| s.to_string()),
                is_nullable: d["is_nullable"]
                    .as_bool()
                    .or_else(|| d["nulls_allowed"].as_bool())
                    .unwrap_or(true),
                initial_default: d["initial_default"].as_str().map(|s| s.to_string()),
                default_value_type: d["default_value_type"].as_str().map(|s| s.to_string()),
                default_value_dialect: d["default_value_dialect"].as_str().map(|s| s.to_string()),
                parent_column: d["parent_column"].as_u64(),
            };
            batch.put(
                keys::key_column(table_id, column_id, begin_snapshot),
                values::encode_value(&row),
            );
        }
        "ducklake_data_file" => {
            let data_file_id = req_u64(d, "data_file_id", table)?;
            let table_id = req_u64(d, "table_id", table)?;
            let begin_snapshot = d["begin_snapshot"]
                .as_u64()
                .or_else(|| d["snapshot_id"].as_u64());
            let row = DataFileRow {
                data_file_id,
                table_id,
                path: req_str(d, "path", table)?,
                file_format: d["file_format"].as_str().unwrap_or("parquet").to_string(),
                record_count: d["record_count"]
                    .as_u64()
                    .or_else(|| d["row_count"].as_u64())
                    .unwrap_or(0),
                file_size_bytes: d["file_size_bytes"].as_u64().unwrap_or(0),
                footer_size: d["footer_size"].as_i64(),
                encryption_key: d["encryption_key"].as_str().map(|s| s.to_string()),
                begin_snapshot,
                end_snapshot: d["end_snapshot"].as_u64(),
                file_order: d["file_order"].as_u64(),
                path_is_relative: d["path_is_relative"].as_bool(),
                row_id_start: d["row_id_start"].as_u64(),
                partition_id: d["partition_id"].as_u64(),
                mapping_id: d["mapping_id"].as_u64(),
                partial_max: d["partial_max"].as_str().map(|s| s.to_string()),
            };
            let encoded = values::encode_value(&row);
            // Primary key.
            batch.put(keys::key_data_file(table_id, data_file_id), encoded.clone());
            // Secondary index — required by list_data_files(). Written
            // atomically with the primary key in the same WriteBatch.
            let idx_begin = begin_snapshot.unwrap_or(0);
            batch.put(
                keys::key_data_file_by_snapshot(table_id, idx_begin, data_file_id),
                encoded.clone(),
            );
            batch.put(
                keys::key_data_file_by_order(
                    table_id,
                    row.file_order.unwrap_or(data_file_id),
                    data_file_id,
                ),
                encoded,
            );
            report.data_file_count += 1;
        }
        "ducklake_delete_file" => {
            let delete_file_id = req_u64(d, "delete_file_id", table)?;
            let data_file_id = req_u64(d, "data_file_id", table)?;
            let row = DeleteFileRow {
                delete_file_id,
                data_file_id,
                path: req_str(d, "path", table)?,
                delete_count: d["delete_count"]
                    .as_u64()
                    .or_else(|| d["row_count"].as_u64())
                    .unwrap_or(0),
                file_size_bytes: d["file_size_bytes"].as_u64().unwrap_or(0),
                snapshot_id: d["snapshot_id"].as_u64().unwrap_or(0),
                table_id: d["table_id"].as_u64(),
                begin_snapshot: d["begin_snapshot"].as_u64(),
                end_snapshot: d["end_snapshot"].as_u64(),
                path_is_relative: d["path_is_relative"].as_bool(),
                format: d["format"].as_str().map(|s| s.to_string()),
                footer_size: d["footer_size"].as_i64(),
                partial_max: d["partial_max"].as_str().map(|s| s.to_string()),
                encryption_key: d["encryption_key"].as_str().map(|s| s.to_string()),
            };
            let encoded = values::encode_value(&row);
            batch.put(
                keys::key_delete_file(data_file_id, delete_file_id),
                encoded.clone(),
            );
            if let Some(table_id) = row.table_id {
                batch.put(
                    keys::key_delete_file_by_table(
                        table_id,
                        row.begin_snapshot.unwrap_or(row.snapshot_id),
                        delete_file_id,
                    ),
                    encoded,
                );
            }
        }
        "ducklake_view" => {
            let view_id = req_u64(d, "view_id", table)?;
            let schema_id = req_u64(d, "schema_id", table)?;
            let begin_snapshot = d["begin_snapshot"].as_u64().unwrap_or(0);
            let row = ViewRow {
                view_id,
                schema_id,
                view_name: req_str(d, "view_name", table)?,
                sql: d["sql"]
                    .as_str()
                    .or_else(|| d["view_definition"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                begin_snapshot,
                end_snapshot: d["end_snapshot"].as_u64(),
                view_uuid: d["view_uuid"].as_str().map(|s| s.to_string()),
                dialect: d["dialect"].as_str().map(|s| s.to_string()),
                column_aliases: d["column_aliases"].as_str().map(|s| s.to_string()),
            };
            batch.put(
                keys::key_view(schema_id, view_id, begin_snapshot),
                values::encode_value(&row),
            );
        }
        "ducklake_macro" => {
            let macro_id = req_u64(d, "macro_id", table)?;
            let schema_id = req_u64(d, "schema_id", table)?;
            let begin_snapshot = d["begin_snapshot"].as_u64().unwrap_or(0);
            let row = MacroRow {
                macro_id,
                schema_id,
                macro_name: req_str(d, "macro_name", table)?,
                macro_type: d["macro_type"].as_str().unwrap_or("scalar").to_string(),
                begin_snapshot,
                end_snapshot: d["end_snapshot"].as_u64(),
                macro_uuid: d["macro_uuid"].as_str().map(|s| s.to_string()),
            };
            batch.put(
                keys::key_macro(schema_id, macro_id, begin_snapshot),
                values::encode_value(&row),
            );
        }
        "ducklake_tag" => {
            let object_id = req_u64(d, "object_id", table)?;
            let tag_key = req_str(d, "tag_key", table)?;
            let begin_snapshot = d["begin_snapshot"].as_u64().unwrap_or(0);
            let tag_key_hash = d["tag_key_hash"]
                .as_u64()
                .unwrap_or_else(|| compute_hash_u64(&tag_key));
            let row = TagRow {
                object_id,
                tag_key,
                tag_value: d["tag_value"].as_str().unwrap_or_default().to_string(),
                begin_snapshot,
                end_snapshot: d["end_snapshot"].as_u64(),
            };
            batch.put(
                keys::key_tag(object_id, tag_key_hash, begin_snapshot),
                values::encode_value(&row),
            );
        }
        "ducklake_column_tag" => {
            let table_id = req_u64(d, "table_id", table)?;
            let column_id = req_u64(d, "column_id", table)?;
            let tag_key = req_str(d, "tag_key", table)?;
            let begin_snapshot = d["begin_snapshot"].as_u64().unwrap_or(0);
            let tag_key_hash = d["tag_key_hash"]
                .as_u64()
                .unwrap_or_else(|| compute_hash_u64(&tag_key));
            let row = ColumnTagRow {
                table_id,
                column_id,
                tag_key,
                tag_value: d["tag_value"].as_str().unwrap_or_default().to_string(),
                begin_snapshot,
                end_snapshot: d["end_snapshot"].as_u64(),
            };
            batch.put(
                keys::key_column_tag(table_id, column_id, tag_key_hash, begin_snapshot),
                values::encode_value(&row),
            );
        }
        "ducklake_snapshot_changes" => {
            let snapshot_id = req_u64(d, "snapshot_id", table)?;
            let row = SnapshotChangesRow {
                snapshot_id,
                change_type: d["change_type"].as_str().unwrap_or_default().to_string(),
                change_info: d["change_info"].as_str().map(|s| s.to_string()),
                schema_id: d["schema_id"].as_u64(),
                table_id: d["table_id"].as_u64(),
                author: d["author"].as_str().map(|s| s.to_string()),
                commit_message: d["commit_message"].as_str().map(|s| s.to_string()),
                commit_extra_info: d["commit_extra_info"].as_str().map(|s| s.to_string()),
                changes_made: d["changes_made"].as_str().map(|s| s.to_string()),
            };
            batch.put(
                keys::key_snapshot_changes(snapshot_id),
                values::encode_value(&row),
            );
        }
        _ => {
            // Unknown table: skip silently.
        }
    }
    Ok(())
}

// ── Helper functions ──────────────────────────────────────────────────────────

#[allow(dead_code)]
fn req_u64(d: &Value, field: &str, table: &str) -> CatalogResult<u64> {
    d[field].as_u64().ok_or_else(|| CatalogError::Import {
        line: 0,
        table: table.to_string(),
        message: format!("missing or invalid u64 field '{field}'"),
    })
}

#[allow(dead_code)]
fn req_str(d: &Value, field: &str, table: &str) -> CatalogResult<String> {
    d[field]
        .as_str()
        .ok_or_else(|| CatalogError::Import {
            line: 0,
            table: table.to_string(),
            message: format!("missing or invalid string field '{field}'"),
        })
        .map(|s| s.to_string())
}

/// Compute a stable u64 hash from a string using DefaultHasher.
///
/// Uses the same algorithm as the private `hash_tag_key` function in
/// `rocklake_catalog::writer` for consistency across migration and write paths.
#[allow(dead_code)]
fn compute_hash_u64(s: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn rusqlite_value_to_json(val: rusqlite::types::Value) -> Value {
    match val {
        rusqlite::types::Value::Null => Value::Null,
        rusqlite::types::Value::Integer(i) => Value::Number(i.into()),
        rusqlite::types::Value::Real(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        rusqlite::types::Value::Text(s) => Value::String(s),
        rusqlite::types::Value::Blob(b) => {
            Value::String(base64::engine::general_purpose::STANDARD.encode(&b))
        }
    }
}

fn normalize_ducklake_row(table: &str, data: &mut Value) {
    let Some(object) = data.as_object_mut() else {
        return;
    };
    let aliases: &[(&str, &str)] = match table {
        "ducklake_metadata" => &[("metadata_key", "key"), ("metadata_value", "value")],
        "ducklake_table" => &[("data_path", "path")],
        "ducklake_column" => &[
            ("column_type", "data_type"),
            ("column_order", "column_index"),
            ("nulls_allowed", "is_nullable"),
        ],
        "ducklake_view" => &[("view_definition", "sql")],
        "ducklake_data_file" => &[("row_count", "record_count")],
        "ducklake_delete_file" => &[("row_count", "delete_count")],
        _ => &[],
    };
    for (source, target) in aliases {
        if !object.contains_key(*target) {
            if let Some(value) = object.remove(*source) {
                object.insert((*target).to_string(), value);
            }
        }
    }
    for field in [
        "is_nullable",
        "path_is_relative",
        "contains_null",
        "contains_nan",
        "is_partition",
    ] {
        if let Some(value) = object.get(field).and_then(Value::as_i64) {
            object.insert(field.to_string(), Value::Bool(value != 0));
        }
    }
}
