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
//!         ↓  import_catalog
//!   RockLake SlateDB catalog
//! ```

use std::collections::HashMap;
use std::io::BufReader;

use base64::Engine as _;
use slatedb::Db;

use serde_json::Value;

use crate::error::{CatalogError, CatalogResult};
use crate::export::ExportedRow;

// ── Version constants ────────────────────────────────────────────────────────

/// DuckLake catalog schema version 7 = V1_0 (the only supported version).
pub const DUCKLAKE_V1_0_CATALOG_VERSION: u64 = rocklake_core::version::DUCKLAKE_CATALOG_VERSION;
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
