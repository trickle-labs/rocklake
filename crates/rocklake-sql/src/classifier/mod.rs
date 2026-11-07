//! SQL statement classifier: pattern-match on AST to identify DuckLake operations.
//!
//! This module is decomposed into sub-modules by concern:
//! - `normalize`: pre-parser AST normalizer (schema prefix stripping, subquery lifting)
//! - `prefix`: pre-parser classifiers for LISTEN/UNLISTEN
//! - `table_selects`: table select classifiers and identifier string helpers
//! - `ast`: AST-based SQL statement classifiers

mod ast;
pub(crate) mod normalize;
mod prefix;
mod table_selects;

use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::error::SqlDispatchError;

use ast::classify_ast;
use prefix::classify_listen_prefix;

/// Fast-path detection for COPY ... FROM STDIN.
/// Returns Some(StatementKind::CopyFromStdin) if the SQL matches.
fn classify_copy_from_stdin(sql: &str) -> Option<StatementKind> {
    let upper = sql.to_uppercase();
    // Match: COPY "public"."ducklake_*" FROM STDIN (FORMAT binary)
    // Also match: COPY public.ducklake_* FROM STDIN
    // Also match: COPY "public"."ducklake_*" (col1, col2) FROM STDIN ...
    if !upper.contains("COPY") || !upper.contains("FROM STDIN") {
        return None;
    }

    // Extract table name between COPY and FROM
    let copy_idx = upper.find("COPY")?;
    let from_idx = upper.find("FROM STDIN")?;
    if from_idx <= copy_idx + 4 {
        return None;
    }

    let table_part = sql[copy_idx + 4..from_idx].trim();
    // Normalize: strip quotes, schema prefix, and column list
    let table = normalize_copy_table(table_part);

    // Accept ducklake_* tables and text-format migration staging tables.
    if table.starts_with("ducklake_")
        || (table.starts_with("update_data_") && upper.contains("FORMAT TEXT"))
    {
        Some(StatementKind::CopyFromStdin { table })
    } else {
        // Unknown table — return None to fall through to sqlparser
        None
    }
}

/// Strip column list from COPY table reference.
/// `"public"."table" (col1, col2)` -> `"public"."table"`
fn strip_column_list(s: &str) -> &str {
    // Find the first '(' that's not inside quotes
    let mut in_quote = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '(' if !in_quote => return s[..i].trim(),
            _ => {}
        }
    }
    s
}

/// Fast-path detection for COPY (SELECT ...) TO STDOUT.
/// Returns Some(StatementKind::CopyToStdout) if the SQL matches.
fn classify_copy_to_stdout(sql: &str) -> Option<StatementKind> {
    let upper = sql.to_uppercase();
    // Match: COPY (SELECT ...) TO STDOUT (FORMAT binary)
    if !upper.contains("COPY") || !upper.contains("TO STDOUT") {
        return None;
    }

    // Check for COPY (...) pattern
    let copy_idx = upper.find("COPY")?;
    let stdout_idx = upper.find("TO STDOUT")?;
    if stdout_idx <= copy_idx + 4 {
        return None;
    }

    // Extract the query between COPY ( and ) TO STDOUT
    let after_copy = &sql[copy_idx + 4..].trim_start();
    if !after_copy.starts_with('(') {
        return None;
    }

    // Find matching closing paren (simple approach: first ')' before TO STDOUT)
    let query_part = &sql[copy_idx + 4..stdout_idx];
    let query_part = query_part.trim();
    if !query_part.starts_with('(') || !query_part.ends_with(')') {
        return None;
    }

    // Extract the inner query
    let inner_query = &query_part[1..query_part.len() - 1].trim();
    let inner_upper = inner_query.to_ascii_uppercase();
    if !inner_upper.starts_with("SELECT") && !inner_upper.starts_with("WITH") {
        return None;
    }

    Some(StatementKind::CopyToStdout {
        query: inner_query.to_string(),
    })
}

/// Normalize a table name from COPY statement.
/// Strips quotes, `public.` schema prefix, and any column list `(col1, col2, ...)`.
fn normalize_copy_table(s: &str) -> String {
    let s = s.trim();

    // Strip column list: find the first '(' that's not inside quotes
    // COPY "public"."table" (col1, col2) -> "public"."table"
    let s = strip_column_list(s);

    // Remove surrounding quotes if present
    let s = s.trim_matches('"');
    // Handle schema-qualified: "public"."table" or public.table
    let s = if let Some(rest) = s.strip_prefix("public.") {
        rest.trim_matches('"')
    } else if let Some(rest) = s.strip_prefix("\"public\".") {
        rest.trim_matches('"')
    } else if s.contains(".") {
        // General case: schema.table
        let parts: Vec<&str> = s.splitn(2, '.').collect();
        if parts.len() == 2 {
            parts[1].trim_matches('"')
        } else {
            s
        }
    } else {
        s
    };
    s.to_lowercase()
}

/// The bounded set of SQL statement shapes supported by RockLake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementKind {
    // ─── Session / Introspection ───────────────────────────────────────
    SelectVersion,
    /// `SELECT version(), (SELECT COUNT(*) FROM pg_settings WHERE name LIKE 'rds%')`
    /// — DuckDB PostgreSQL connector checks for AWS RDS
    SelectVersionWithRdsCheck,
    /// `SELECT 1` — Health check query
    SelectOne,
    SelectCurrentSchema,
    SelectCurrentDatabase,
    SelectPgType,
    /// DuckDB 2.0 probes pg_catalog.pg_tables for inlined-deletion tables.
    SelectPgTables,
    ShowVariable(String),
    SetVariable(String, String),

    // ─── Session / Connection Management ──────────────────────────────
    /// `DISCARD ALL` / `DISCARD SEQUENCES` / `DISCARD PLANS` / `DISCARD TEMP`
    /// — DuckDB sends this when returning a connection to the pool.
    DiscardAll,
    /// `SELECT to_regclass('duckdb_secrets')` — DuckDB secret storage fast-path
    /// check. Returns `NULL` because RockLake has no `duckdb_secrets` table.
    SelectToRegclass,
    /// `SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE ...)`
    /// — DuckDB secret storage fallback check. Returns `false`.
    SelectExistsInfoSchema,
    /// `SELECT pg_database_size(current_database())` — informational size query.
    SelectPgDatabaseSize,
    /// Multi-statement catalog scan sent by the DuckDB postgres scanner as a
    /// single `PQsendQuery` call:
    /// `BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ;
    ///  SELECT oid, nspname FROM pg_namespace ...;
    ///  SELECT ... FROM pg_class JOIN ... UNION ALL ...;
    ///  SELECT ... FROM pg_enum JOIN ...;
    ///  SELECT ... FROM pg_type JOIN ...;
    ///  SELECT ... FROM pg_indexes JOIN ...;
    ///  ROLLBACK;`
    /// Returns five result sets in sequence.
    PgCatalogScan,

    // ─── Transaction Control ───────────────────────────────────────────
    Begin,
    Commit,
    Rollback,

    // ─── DuckLake Read Operations ──────────────────────────────────────
    SelectMaxSnapshot,
    /// Full latest-snapshot tuple used by DuckLake metadata manager:
    /// `SELECT snapshot_id, schema_version, next_catalog_id, next_file_id FROM ducklake_snapshot ...`
    SelectLatestSnapshotInfo,
    SelectSnapshotStatsAndChanges,
    /// Full history projection used while backfilling inlined schema versions.
    SelectSnapshotHistory,
    /// `SELECT max(snapshot_id) FROM ducklake_snapshot WHERE snapshot_id > $1`
    SelectMaxSnapshotAfter,
    SelectSchemas,
    SelectTables,
    SelectColumns,
    SelectDataFiles,
    /// DuckDB 2.0 CTE data-file listing for mutations.
    SelectExtendedDataFiles,
    /// `SELECT ... FROM ducklake_data_file ... LIMIT $N` (parameterized limit)
    SelectDataFilesWithLimit,
    SelectDeleteFiles,
    SelectFileColumnStats,
    SelectTableStats,
    SelectTableColumnStats,
    SelectMetadata,
    SelectSnapshot,
    /// `SELECT ... FROM ducklake_snapshot ORDER BY snapshot_id ASC LIMIT 1`
    SelectFirstSnapshot,
    /// `SELECT * FROM ducklake_snapshot_changes`
    SelectSnapshotChanges,
    SelectInlinedData,
    SelectViews,
    SelectMacros,
    SelectMacroImpls,
    SelectMacroParameters,
    /// `SELECT gen_random_uuid()` — pg-tide-relay generates UUIDs
    SelectGenRandomUuid,
    /// `SELECT ducklake_latest_snapshot_id($1::regclass)` — pg-trickle CDC
    /// startup handshake: resolves the latest snapshot boundary for a named
    /// table before calling `table_changes()`.  The table argument is the
    /// qualified table name cast to `regclass` (e.g. `'lake.events'::regclass`).
    ///
    /// Returns a single BIGINT column `ducklake_latest_snapshot_id` containing
    /// the `snapshot_id` of the latest visible snapshot, or NULL if no snapshot
    /// has been committed yet.
    SelectLatestSnapshotId,

    // ─── v0.27 DuckLake Facade Tables ─────────────────────────────────
    /// `SELECT * FROM ducklake_tag`
    SelectTags,
    /// `SELECT * FROM ducklake_column_tag`
    SelectColumnTags,
    /// `SELECT * FROM ducklake_sort_info`
    SelectSortInfo,
    /// `SELECT * FROM ducklake_schema_version`
    SelectSchemaVersion,
    SelectDuckLakeMetadataTable {
        table_name: String,
    },

    // ─── DuckLake Write Operations ─────────────────────────────────────
    InsertSnapshot,
    InsertSnapshotChanges,
    InsertSchema,
    InsertTable,
    InsertColumn,
    InsertDataFile,
    InsertDeleteFile,
    InsertTableStats,
    InsertTableColumnStats,
    InsertFileColumnStats,
    InsertMetadata,
    InsertInlinedDataTables,
    InsertSchemaVersions,
    InsertView,
    InsertMacro,
    InsertMacroImpl,
    InsertMacroParameters,

    UpdateEndSnapshot(String),
    UpdateMetadata,
    UpdateTableStats,
    UpdateTableColumnStats,
    /// A DuckLake v1.0-to-v1.1 migration statement whose effect is applied
    /// atomically when the version marker is updated.
    DuckLakeV11MigrationStep,

    // ─── Inlined Data DDL/DML ──────────────────────────────────────────
    CreateInlinedTable,
    InsertInlinedRow,
    UpdateInlinedRowEndSnapshot,
    SelectInlinedRows,

    // ─── Virtual Catalog SQL Tables ────────────────────────────────────
    /// `SELECT * FROM rocklake_catalog.{table_name}` — read-only catalog introspection.
    /// Mutations against `rocklake_catalog.*` return SQLSTATE 25006.
    VirtualCatalogScan {
        table_name: String,
    },
    /// `INSERT`/`UPDATE`/`DELETE` against `rocklake_catalog.*` — rejected with
    /// SQLSTATE 25006 (read-only transaction). The virtual catalog is read-only;
    /// all write paths must go through the dedicated write operations.
    VirtualCatalogMutation {
        table_name: String,
    },

    // ─── v0.18 DuckLake Standard Interface ─────────────────────────────
    /// `SELECT * FROM table_changes('schema.table', start_snapshot := N, end_snapshot := M)`
    TableChanges {
        table_ref: String,
        start_snapshot: u64,
        end_snapshot: u64,
    },
    /// `SELECT rocklake.next_rowid_range('schema.table', count := N)`
    NextRowidRange {
        table_ref: String,
        count: u64,
    },
    /// `SELECT rocklake.hold_snapshot(min_snapshot_id := N, consumer_id := '...', ttl_seconds := N)`
    HoldSnapshot {
        min_snapshot_id: u64,
        consumer_id: String,
        ttl_seconds: u64,
    },
    /// `SELECT rocklake.release_snapshot(consumer_id := '...')`
    ReleaseSnapshot {
        consumer_id: String,
    },
    /// `LISTEN channel`
    Listen {
        channel: String,
    },
    /// `UNLISTEN channel`
    Unlisten {
        channel: String,
    },
    /// `CREATE TABLE IF NOT EXISTS <extension_schema>.<table> (...)`
    CreateExtensionTable {
        schema_name: String,
        table_name: String,
    },
    /// `INSERT INTO <extension_schema>.<table> (...) VALUES (...)`
    InsertExtensionRow {
        schema_name: String,
        table_name: String,
        columns: Vec<String>,
        values_json: String,
    },
    /// `SELECT ... FROM <extension_schema>.<table>`
    SelectExtensionTable {
        schema_name: String,
        table_name: String,
    },
    /// `DELETE FROM <extension_schema>.<table> WHERE ...`
    DeleteExtensionRows {
        schema_name: String,
        table_name: String,
    },
    /// `DELETE FROM "public".ducklake_inlined_data_<table_id>_<schema_version> WHERE ctid IN (...)`
    /// Issued by DuckLake CHECKPOINT to clean up flushed inlined rows.
    DeleteInlinedDataRows {
        table_name: String,
    },
    /// `DELETE FROM "public".ducklake_data_file WHERE data_file_id IN (...)`
    /// or similar for other DuckLake catalog tables during CHECKPOINT garbage collection.
    DeleteDuckLakeCatalogRows {
        table_name: String,
    },

    // ─── COPY Protocol ─────────────────────────────────────────────────
    /// `COPY "public"."ducklake_*" FROM STDIN (FORMAT binary)`
    /// DuckDB 1.5+ uses binary COPY for catalog initialization.
    /// RockLake accepts and discards the payload (no-op).
    CopyFromStdin {
        table: String,
    },
    /// `COPY (SELECT ...) TO STDOUT (FORMAT binary)`
    /// DuckDB 1.5+ uses binary COPY to read catalog data.
    CopyToStdout {
        /// The embedded SQL query to execute.
        query: String,
    },

    // ─── Unsupported ───────────────────────────────────────────────────
    Unsupported(String),
}

/// Classify a SQL string into a `StatementKind`.
pub fn classify_statement(sql: &str) -> Result<StatementKind, SqlDispatchError> {
    // Apply AST normalization first: strip schema prefixes etc.
    let sql_cow = normalize::normalize_sql(sql);
    let sql = sql_cow.as_ref();
    let lower = sql.to_ascii_lowercase();

    if is_ducklake_v11_metadata_staging_update(&lower) {
        return Ok(StatementKind::UpdateMetadata);
    }
    if is_ducklake_v11_migration_step(&lower) {
        return Ok(StatementKind::DuckLakeV11MigrationStep);
    }

    // DuckLake 1.0 on DuckDB 2.0 probes pg_tables before creating its
    // optional inlined-deletion table. This is a PostgreSQL system view.
    if lower.trim_start().starts_with("select") && lower.contains("pg_catalog.pg_tables") {
        return Ok(StatementKind::SelectPgTables);
    }

    // Pre-parse fast path for LISTEN/UNLISTEN.
    if let Some(result) = classify_listen_prefix(sql) {
        return result;
    }

    // Match binary COPY before DELETE: DuckDB 2.0's COPY (WITH ... JOIN
    // ducklake_delete_file ...) contains "delete" in a relation name.
    if let Some(kind) = classify_copy_from_stdin(sql) {
        return Ok(kind);
    }
    if let Some(kind) = classify_copy_to_stdout(sql) {
        return Ok(kind);
    }

    // DuckDB 2.0 uses CTEs for both ordinary scans and UPDATE/DELETE.
    // Only the extended file projection includes a delete ID and row count.
    if lower.trim_start().starts_with("with ")
        && lower.contains("ducklake_file_column_stats")
        && lower.contains("ducklake_data_file")
        && lower.contains("ducklake_delete_file")
    {
        return Ok(
            if lower.contains("del.delete_file_id") && lower.contains("data.record_count") {
                StatementKind::SelectExtendedDataFiles
            } else {
                StatementKind::SelectDataFiles
            },
        );
    }

    if lower.contains("update") && lower.contains("ducklake_table_column_stats") {
        return Ok(StatementKind::UpdateTableColumnStats);
    }

    if lower.contains("update") && lower.contains("ducklake_inlined_data_") {
        return Ok(StatementKind::UpdateInlinedRowEndSnapshot);
    }

    // DuckDB drops superseded internal inlined tables after a schema change.
    // They are represented by catalog rows here, not physical SQL tables.
    if lower.trim_start().starts_with("drop table") && lower.contains("ducklake_inlined_data_") {
        return Ok(StatementKind::CreateInlinedTable);
    }

    // Pre-parse fast path for DELETE statements on DuckLake catalog tables
    if lower.trim_start().starts_with("delete")
        && lower.contains("from")
        && lower.contains("ducklake_")
        && !lower.contains("rocklake_catalog.")
    {
        // Try to extract the first DELETE statement from the batch
        if let Some(first_delete_idx) = lower.find("delete") {
            // Extract from first DELETE onwards until we hit the next semicolon or end
            let delete_stmt = if let Some(semi_idx) = sql[first_delete_idx..].find(';') {
                &sql[first_delete_idx..first_delete_idx + semi_idx]
            } else {
                &sql[first_delete_idx..]
            };

            // Simple approach: find "ducklake_" and extract the table name
            if let Some(ducklake_idx) = delete_stmt.to_lowercase().find("ducklake_") {
                // Extract from "ducklake_" onwards until we hit whitespace, comma, or newline
                let remaining = &delete_stmt[ducklake_idx..];
                let table_name = remaining
                    .split(|c: char| {
                        c.is_whitespace() || c == ',' || c == ')' || c == ';' || c == '"'
                    })
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

                if table_name.contains("ducklake_") {
                    if table_name.contains("inlined_data_")
                        && table_name != "ducklake_inlined_data_tables"
                    {
                        return Ok(StatementKind::DeleteInlinedDataRows { table_name });
                    } else {
                        return Ok(StatementKind::DeleteDuckLakeCatalogRows { table_name });
                    }
                } else {
                    return Err(SqlDispatchError::ParseError(format!(
                        "PREPARSE_DELETE: table_name did not match pattern: {}",
                        table_name
                    )));
                }
            } else {
                return Err(SqlDispatchError::ParseError(
                    "PREPARSE_DELETE: ducklake_ not found in delete_stmt".to_string(),
                ));
            }
        } else {
            return Err(SqlDispatchError::ParseError(
                "PREPARSE_DELETE: DELETE not found".to_string(),
            ));
        }
    }

    // `SET TIME ZONE <value>` is a PostgreSQL-specific syntax that sqlparser
    // may emit as a non-SetVariable AST node.  Map it to SetVariable so
    // drivers that send this form (e.g. pgcli, psycopg3) are accepted.
    if lower.starts_with("set time zone") {
        let tz = sql["set time zone".len()..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        return Ok(StatementKind::SetVariable("TimeZone".to_string(), tz));
    }

    if lower.contains("union all")
        && lower.contains("ducklake_snapshot")
        && lower.contains("ducklake_snapshot_changes")
        && lower.contains("ducklake_table_stats")
        && lower.contains("ducklake_table_column_stats")
    {
        return Ok(StatementKind::SelectSnapshotStatsAndChanges);
    }

    // Pre-parse detection for the multi-statement pg_namespace catalog scan.
    // DuckDB postgres scanner sends this as a single PQsendQuery call; sqlparser
    // would only see the leading BEGIN. Detect by characteristic tokens.
    if sql.contains("pg_namespace") && sql.contains("pg_class") {
        return Ok(StatementKind::PgCatalogScan);
    }

    // Pre-parse fast path for COPY ... FROM STDIN (binary).
    // DuckDB 1.5+ uses binary COPY for catalog initialization.
    // sqlparser doesn't handle the protocol-level COPY syntax, so we detect it here.
    if let Some(kind) = classify_copy_from_stdin(sql) {
        return Ok(kind);
    }

    // Pre-parse fast path for COPY (SELECT ...) TO STDOUT (binary).
    // DuckDB 1.5+ uses binary COPY to read catalog data.
    if let Some(kind) = classify_copy_to_stdout(sql) {
        return Ok(kind);
    }

    let dialect = PostgreSqlDialect {};
    let statements = Parser::parse_sql(&dialect, sql)
        .map_err(|e| SqlDispatchError::ParseError(e.to_string()))?;

    if statements.is_empty() {
        return Err(SqlDispatchError::ParseError("empty statement".to_string()));
    }

    // Attempt to lift trivial subquery wrappers added by some client drivers.
    let stmt = &statements[0];
    if let Some(lifted) = normalize::try_lift_trivial_subquery(stmt) {
        return Ok(classify_ast(&lifted));
    }

    Ok(classify_ast(stmt))
}

fn is_ducklake_v11_migration_step(sql: &str) -> bool {
    let inlined_table = sql.contains("ducklake_inlined_data_");
    let migration_temp_table = sql.starts_with("create local temporary table")
        && sql.contains("update_data_")
        && sql.contains("__page_id_string")
        && (sql.contains("initial_default") || sql.contains("\"value\" varchar"));
    let struct_default_temp_update = sql.starts_with("update")
        && sql.contains("ducklake_column")
        && sql.contains("update_data_")
        && sql.contains("initial_default")
        && sql.contains("__page_id_string");
    let inlined_column_rename = sql.contains("alter table")
        && inlined_table
        && sql.contains("rename column")
        && [
            ("row_id", "_ducklake_row_id"),
            ("begin_snapshot", "_ducklake_begin_snapshot"),
            ("end_snapshot", "_ducklake_end_snapshot"),
        ]
        .iter()
        .any(|(old, new)| sql.contains(old) && sql.contains(new));
    let added_column = [
        ("ducklake_data_file", "row_group_count"),
        ("ducklake_delete_file", "row_group_count"),
        ("ducklake_file_column_stats", "min_is_exact"),
        ("ducklake_file_column_stats", "max_is_exact"),
        ("ducklake_table_column_stats", "min_is_exact"),
        ("ducklake_table_column_stats", "max_is_exact"),
        ("ducklake_schema", "parent_schema_id"),
    ]
    .iter()
    .any(|(table, column)| {
        sql.starts_with("alter table")
            && sql.contains(table)
            && sql.contains("add column")
            && sql.contains(column)
    });
    inlined_column_rename
        || migration_temp_table
        || struct_default_temp_update
        || (sql.starts_with("alter table")
            && ((inlined_table && sql.contains("_migrated")) || added_column))
        || (sql.starts_with("create table") && inlined_table && sql.contains("_migrated"))
        || (sql.starts_with("insert into")
            && ((sql.contains("ducklake_schema_versions") && sql.contains(" select "))
                || (inlined_table && sql.contains("_migrated") && sql.contains(" select "))))
        || (sql.starts_with("update")
            && ((sql.contains("update ducklake_column as renamed")
                && sql.contains("from ducklake_column as original")
                && sql.contains("initial_default"))
                || ((sql.contains("update ducklake_file_column_stats as stats")
                    || sql.contains("update ducklake_table_column_stats as stats"))
                    && sql.contains("set min_value"))))
}

fn is_ducklake_v11_metadata_staging_update(sql: &str) -> bool {
    sql.starts_with("update")
        && sql.contains("ducklake_metadata")
        && sql.contains("update_data_")
        && sql.contains("__page_id_string")
        && sql.contains("ctid")
        && sql.contains("value")
}

#[cfg(test)]
mod duckdb_2_copy_tests {
    use super::*;

    #[test]
    fn cte_copy_is_a_read_only_copy_not_a_catalog_delete() {
        let sql = r#"COPY (WITH col_1_stats AS (
            SELECT data_file_id FROM "public".ducklake_file_column_stats
            WHERE column_id = 1
        ) SELECT data.data_file_id, del.delete_file_id, data.record_count,
                 data.path AS data_path, del.path AS del_path
          FROM "public".ducklake_data_file data
        LEFT JOIN "public".ducklake_delete_file del
        ON data.data_file_id = del.data_file_id
        ) TO STDOUT (FORMAT "binary");"#;
        let kind = classify_statement(sql).expect("valid DuckDB 2.0 COPY");
        let StatementKind::CopyToStdout { query } = kind else {
            panic!("expected COPY TO STDOUT, got {kind:?}");
        };
        assert!(matches!(
            classify_statement(&query).unwrap(),
            StatementKind::SelectExtendedDataFiles
        ));
    }

    #[test]
    fn cte_copy_without_extended_columns_uses_regular_scan() {
        let sql = r#"COPY (WITH col_1_stats AS (
            SELECT data_file_id FROM "public".ducklake_file_column_stats
            WHERE column_id = 1
        ) SELECT data.data_file_id, data.path AS data_path,
                 del.path AS del_path
          FROM "public".ducklake_data_file data
          LEFT JOIN "public".ducklake_delete_file del
          ON data.data_file_id = del.data_file_id
        ) TO STDOUT (FORMAT "binary");"#;
        let kind = classify_statement(sql).expect("valid DuckDB 2.0 COPY");
        let StatementKind::CopyToStdout { query } = kind else {
            panic!("expected COPY TO STDOUT, got {kind:?}");
        };
        assert!(matches!(
            classify_statement(&query).unwrap(),
            StatementKind::SelectDataFiles
        ));
    }

    #[test]
    fn ducklake_v11_migration_sql_is_scoped_to_known_steps() {
        assert!(matches!(
            classify_statement(
                "ALTER TABLE public.ducklake_schema ADD COLUMN parent_schema_id BIGINT"
            )
            .unwrap(),
            StatementKind::DuckLakeV11MigrationStep
        ));
        assert!(matches!(
            classify_statement(
                "UPDATE public.ducklake_metadata SET value = '1.1' WHERE key = 'version'"
            )
            .unwrap(),
            StatementKind::UpdateMetadata
        ));
        assert!(matches!(
            classify_statement("ALTER TABLE ducklake_schema ADD COLUMN unrelated VARCHAR").unwrap(),
            StatementKind::Unsupported(_)
        ));
        assert!(matches!(
            classify_statement(
                "CREATE TABLE public.ducklake_view_column_tag(view_id BIGINT, column_name VARCHAR)"
            )
            .unwrap(),
            StatementKind::CreateInlinedTable
        ));
        assert!(matches!(
            classify_statement(
                "SET LOCAL extra_float_digits = 3; \
                 ALTER TABLE public.ducklake_inlined_data_2_2 RENAME COLUMN row_id TO _ducklake_row_id; \
                 ALTER TABLE public.ducklake_inlined_data_2_2 RENAME COLUMN begin_snapshot TO _ducklake_begin_snapshot; \
                 ALTER TABLE public.ducklake_inlined_data_2_2 RENAME COLUMN end_snapshot TO _ducklake_end_snapshot"
            )
            .unwrap(),
            StatementKind::DuckLakeV11MigrationStep
        ));
        assert!(matches!(
            classify_statement("SELECT snapshot_id, schema_version FROM ducklake_snapshot")
                .unwrap(),
            StatementKind::SelectSnapshotHistory
        ));
        assert!(matches!(
            classify_statement(
                "CREATE LOCAL TEMPORARY TABLE \"update_data_123\"(\"initial_default\" VARCHAR, \
                 \"default_value\" VARCHAR, __page_id_string VARCHAR) ON COMMIT DROP"
            )
            .unwrap(),
            StatementKind::DuckLakeV11MigrationStep
        ));
        assert!(matches!(
            classify_statement(
                "CREATE LOCAL TEMPORARY TABLE \"update_data_123\"(\"value\" VARCHAR, \
                 __page_id_string VARCHAR) ON COMMIT DROP"
            )
            .unwrap(),
            StatementKind::DuckLakeV11MigrationStep
        ));
        assert!(matches!(
            classify_statement("COPY \"update_data_123\" FROM STDIN (FORMAT TEXT)").unwrap(),
            StatementKind::CopyFromStdin { table } if table == "update_data_123"
        ));
        assert!(matches!(
            classify_statement(
                "UPDATE public.ducklake_column SET initial_default = update_data_123.initial_default \
                 FROM update_data_123 WHERE ducklake_column.ctid=__page_id_string::TID"
            )
            .unwrap(),
            StatementKind::DuckLakeV11MigrationStep
        ));
        assert!(matches!(
            classify_statement(
                "UPDATE public.ducklake_metadata SET value = update_data_123.value \
                 FROM update_data_123 WHERE ducklake_metadata.ctid=__page_id_string::TID"
            )
            .unwrap(),
            StatementKind::UpdateMetadata
        ));
    }
}
