//! SQL executor: translates classified SQL into CatalogStore operations.
//!
//! This module is decomposed into sub-modules by feature family:
//! - `helpers`: shared response builders and parameter utilities
//! - `catalog`: catalog read/write operations and execute_commit
//! - `extension`: extension schema operations
//! - `session`: snapshot lease operations
//! - `meta`: VirtualCatalogScan and info_schema operations

mod catalog;
mod extension;
mod helpers;
mod meta;
mod session;

pub(crate) fn extended_data_file_fields() -> Vec<pgwire::api::results::FieldInfo> {
    catalog::extended_data_file_fields()
}

pub(crate) fn scan_data_file_fields(sql: &str) -> Vec<pgwire::api::results::FieldInfo> {
    catalog::scan_data_file_fields(sql)
}

use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use pgwire::api::results::{CopyResponse, Response, Tag};
use sqlparser::ast::{visit_expressions, BinaryOperator, Expr, SetExpr, Statement};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use rocklake_catalog::CatalogStore;
use rocklake_sql::{classify_statement, ParamValues, StatementKind};

use crate::error::RockLakeError;
use crate::notify::NotifyManager;
use crate::session::{BufferedOp, CopyAccumulator, SessionState};

use catalog::{
    execute_commit, execute_next_rowid_range, execute_table_changes, make_column_mapping_response,
    make_column_tags_response, make_columns_response, make_data_files_stream_response,
    make_data_files_stream_response_from_rows, make_delete_files_stream_response,
    make_extended_data_file_response, make_file_column_stats_stream_response,
    make_file_ids_response, make_file_partition_values_response, make_file_variant_stats_response,
    make_files_scheduled_for_deletion_response, make_global_table_stats_response,
    make_inlined_data_tables_response, make_inlined_deletes_response, make_inlined_rows_response,
    make_latest_snapshot_info_response, make_macro_impls_response, make_macro_parameters_response,
    make_macros_response, make_metadata_response, make_metadata_table_empty_response,
    make_name_mapping_response, make_partition_columns_response, make_partition_info_response,
    make_scan_data_file_response, make_schema_version_response, make_schema_versions_response,
    make_schemas_response, make_snapshot_changes_response, make_snapshot_history_response,
    make_snapshot_row_response, make_snapshot_stats_changes_response,
    make_sort_expressions_response, make_sort_info_response, make_table_column_stats_response,
    make_table_stats_rows_response_for_sql, make_tables_response, make_tags_response,
    make_view_column_tags_response, make_views_response, parse_inlined_table_ids,
};
use extension::{
    execute_create_extension_table, execute_delete_extension_rows, execute_insert_extension_row,
    execute_select_extension_table,
};
use helpers::{
    apply_set, get_show_value, get_snapshot_param, make_empty_response, make_false_bool_response,
    make_pg_catalog_inlined_table_response, make_pg_catalog_scan_responses,
    make_pg_catalog_staleness_response, make_pg_tables_probe_response, make_pg_type_response,
    make_single_int_response, make_single_text_response, make_version_with_rds_check_response,
    require_param_u64, resolve_reader,
};
use meta::execute_virtual_catalog_scan;
use session::{execute_hold_snapshot, execute_release_snapshot};

/// Access mode enforced before any statement reaches catalog I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Writer,
    Reader,
}

fn is_mutating_statement(kind: &StatementKind, sql: &str) -> bool {
    match kind {
        StatementKind::InsertSnapshot
        | StatementKind::InsertSnapshotChanges
        | StatementKind::InsertSchema
        | StatementKind::InsertTable
        | StatementKind::InsertColumn
        | StatementKind::InsertDataFile
        | StatementKind::InsertDeleteFile
        | StatementKind::InsertTableStats
        | StatementKind::InsertTableColumnStats
        | StatementKind::InsertFileColumnStats
        | StatementKind::InsertMetadata
        | StatementKind::UpdateMetadata
        | StatementKind::DuckLakeV11MigrationStep
        | StatementKind::InsertInlinedDataTables
        | StatementKind::InsertSchemaVersions
        | StatementKind::InsertView
        | StatementKind::InsertMacro
        | StatementKind::InsertMacroImpl
        | StatementKind::InsertMacroParameters
        | StatementKind::UpdateEndSnapshot(_)
        | StatementKind::UpdateTableStats
        | StatementKind::UpdateTableColumnStats
        | StatementKind::CreateInlinedTable
        | StatementKind::InsertInlinedRow
        | StatementKind::UpdateInlinedRowEndSnapshot
        | StatementKind::VirtualCatalogMutation { .. }
        | StatementKind::CreateExtensionTable { .. }
        | StatementKind::InsertExtensionRow { .. }
        | StatementKind::DeleteExtensionRows { .. }
        | StatementKind::DeleteInlinedDataRows { .. }
        | StatementKind::DeleteDuckLakeCatalogRows { .. }
        | StatementKind::CopyFromStdin { .. }
        | StatementKind::NextRowidRange { .. }
        | StatementKind::HoldSnapshot { .. }
        | StatementKind::ReleaseSnapshot { .. } => true,
        StatementKind::Unsupported(_) => {
            let lower = sql.trim_start().to_ascii_lowercase();
            matches!(
                lower.split_whitespace().next(),
                Some("insert" | "update" | "delete" | "create" | "alter" | "drop" | "truncate")
            ) || (lower.starts_with("copy ") && lower.contains(" from "))
        }
        _ => false,
    }
}

fn ensure_access(mode: AccessMode, kind: &StatementKind, sql: &str) -> Result<(), RockLakeError> {
    if mode == AccessMode::Reader && is_mutating_statement(kind, sql) {
        return Err(RockLakeError::ReadOnlyReplica);
    }
    Ok(())
}

async fn submit_ops(
    ops: Vec<BufferedOp>,
    session: &mut SessionState,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
    notify_manager: &Arc<NotifyManager>,
) -> Result<usize, RockLakeError> {
    let row_count = ops.len();
    if session.in_transaction {
        for op in ops {
            session.pending_txn.push(op)?;
        }
    } else {
        execute_commit(ops, store, notify_manager).await?;
    }
    Ok(row_count)
}

async fn stable_ducklake_v11(
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<bool, RockLakeError> {
    let catalog = store.lock().await;
    crate::schema_registry::is_ducklake_v11(&catalog)
        .await
        .map_err(RockLakeError::from)
}

async fn current_ducklake_version(
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<Option<String>, RockLakeError> {
    store
        .lock()
        .await
        .read_latest()
        .get_metadata(rocklake_core::keys::MetadataScope::Global, 0, "version")
        .await
        .map(|row| row.map(|metadata| metadata.value))
        .map_err(RockLakeError::from)
}

fn required_param_u64(
    params: &ParamValues,
    index: usize,
    name: &str,
) -> Result<u64, RockLakeError> {
    params
        .get_u64(index)
        .map_err(|_| RockLakeError::MissingParam {
            name: name.to_string(),
        })
}

fn required_param_i64(
    params: &ParamValues,
    index: usize,
    name: &str,
) -> Result<i64, RockLakeError> {
    params
        .get_i64(index)
        .map_err(|_| RockLakeError::MissingParam {
            name: name.to_string(),
        })
}

fn required_param_string(
    params: &ParamValues,
    index: usize,
    name: &str,
) -> Result<String, RockLakeError> {
    params
        .get_string(index)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RockLakeError::MissingParam {
            name: name.to_string(),
        })
}

fn required_param_bool(
    params: &ParamValues,
    index: usize,
    name: &str,
) -> Result<bool, RockLakeError> {
    params
        .get_bool(index)
        .map_err(|_| RockLakeError::MissingParam {
            name: name.to_string(),
        })
}

/// Execute a SQL statement against the catalog, returning PG wire responses.
pub async fn execute_sql<'a>(
    sql: &'a str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
    session: &mut SessionState,
    notify_manager: &Arc<NotifyManager>,
    extension_schemas: &Arc<Vec<String>>,
) -> Result<Vec<Response<'a>>, RockLakeError> {
    execute_sql_with_mode(
        sql,
        params,
        store,
        session,
        notify_manager,
        extension_schemas,
        AccessMode::Writer,
    )
    .await
}

/// Execute a SQL statement with an immutable access mode.
pub async fn execute_sql_with_mode<'a>(
    sql: &'a str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
    session: &mut SessionState,
    notify_manager: &Arc<NotifyManager>,
    extension_schemas: &Arc<Vec<String>>,
    mode: AccessMode,
) -> Result<Vec<Response<'a>>, RockLakeError> {
    let result = execute_sql_inner(
        sql,
        params,
        store,
        session,
        notify_manager,
        extension_schemas,
        mode,
    )
    .await;
    if result.is_err() {
        // A failed statement aborts the logical DuckLake transaction. No
        // earlier buffered operation may become visible if a later statement
        // in the same transaction fails.
        session.pending_txn.clear();
        session.bootstrap = Default::default();
        session.metadata_ctid_rows.clear();
        session.inlined_delete_ctid_rows.clear();
        session.in_transaction = false;
        session.transaction_snapshot_id = None;
        session.ducklake_v11_migration_in_progress = false;
    }
    result
}

async fn execute_sql_inner<'a>(
    sql: &'a str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
    session: &mut SessionState,
    notify_manager: &Arc<NotifyManager>,
    extension_schemas: &Arc<Vec<String>>,
    mode: AccessMode,
) -> Result<Vec<Response<'a>>, RockLakeError> {
    // Some protocol clients send the PostgreSQL catalog scan as one logical
    // classifier input even though it contains several SQL statements.
    if let Ok(kind @ StatementKind::PgCatalogScan) = classify_statement(sql) {
        ensure_access(mode, &kind, sql)?;
        return execute_classified(
            kind,
            sql,
            params,
            store,
            session,
            notify_manager,
            extension_schemas,
            mode,
        )
        .await;
    }

    if let Some(statements) = parse_multi_statement_batch(sql) {
        let mut all_responses = Vec::new();
        for statement_sql in statements {
            let kind = classify_statement(&statement_sql)?;
            ensure_access(mode, &kind, &statement_sql)?;
            let mut responses = execute_classified(
                kind,
                &statement_sql,
                params,
                store,
                session,
                notify_manager,
                extension_schemas,
                mode,
            )
            .await?;
            all_responses.append(&mut responses);
        }
        return Ok(all_responses);
    }

    let kind = classify_statement(sql)?;
    ensure_access(mode, &kind, sql)?;
    execute_classified(
        kind,
        sql,
        params,
        store,
        session,
        notify_manager,
        extension_schemas,
        mode,
    )
    .await
}

fn parse_multi_statement_batch(sql: &str) -> Option<Vec<String>> {
    if !sql.contains(';') {
        return None;
    }

    let dialect = PostgreSqlDialect {};
    let statements = Parser::parse_sql(&dialect, sql).ok()?;
    (statements.len() > 1).then(|| {
        statements
            .into_iter()
            .map(|stmt| stmt.to_string())
            .collect()
    })
}

fn literal_insert_values(sql: &str) -> Vec<Option<String>> {
    literal_insert_rows(sql)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// Extract row IDs from a DuckLake CHECKPOINT DELETE:
///   `DELETE FROM ... WHERE ctid IN ('(524288,0)', '(131073,0)')`
/// Each ctid is `(block_number, tuple_index)`; RockLake encodes row_id as block_number.
fn row_ids_from_ctid_sql(sql: &str) -> Vec<u64> {
    let mut ids = Vec::new();
    let mut rest = sql;
    while let Some(pos) = rest.find('(') {
        rest = &rest[pos + 1..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let num = &rest[..end];
        if !num.is_empty() {
            if let Ok(n) = num.parse::<u64>() {
                if rest[end..].starts_with(',') {
                    ids.push(n);
                }
            }
        }
    }
    ids
}

fn inlined_data_table_ids_from_ctid_sql(sql: &str) -> Vec<(u64, u64)> {
    let mut ids = Vec::new();
    let mut rest = sql;
    while let Some(open) = rest.find("'(") {
        rest = &rest[open + 2..];
        let Some(comma) = rest.find(',') else { break };
        let Some(close) = rest.find(")'") else { break };
        let table_id = rest[..comma].trim().parse::<u64>().ok();
        let schema_version = rest[comma + 1..close].trim().parse::<u64>().ok();
        if let (Some(table_id), Some(schema_version)) = (table_id, schema_version) {
            ids.push((table_id, schema_version));
        }
        rest = &rest[close + 2..];
    }
    ids
}

/// Extract file IDs from a DuckLake CHECKPOINT catalog DELETE:
///   `DELETE FROM "public".ducklake_data_file WHERE data_file_id IN (1, 2);`
/// Handles various DuckLake catalog tables that need garbage collection.
fn file_ids_from_where_sql(sql: &str) -> Vec<u64> {
    let mut ids = Vec::new();
    let normalized_sql = sql.replace(['\n', '\r', '\t'], " ");
    let lower = normalized_sql.to_lowercase();

    // Find the WHERE clause
    let Some(where_idx) = lower.find(" where ") else {
        return ids;
    };
    let after_where = &normalized_sql[where_idx + 7..];
    let lower_after = after_where.to_lowercase();

    // Pattern 1: data_file_id IN (id1, id2, ...) or delete_file_id IN (...)
    // Accept both `IN (...)` and `IN(...)` formatting.
    let in_patterns = [" in (", "in (", " in(", "in("];
    for in_pattern in &in_patterns {
        if let Some(in_idx) = lower_after.find(in_pattern) {
            let after_in = &after_where[in_idx + in_pattern.len()..];
            if let Some(close_paren) = after_in.find(')') {
                let id_list = &after_in[..close_paren];
                // Split by comma and parse each ID
                for part in id_list.split(',') {
                    let trimmed = part.trim().trim_matches('\'').trim_matches('"');
                    if let Ok(id) = trimmed.parse::<u64>() {
                        ids.push(id);
                    }
                }
                if !ids.is_empty() {
                    return ids; // Found IDs, return early
                }
            }
        }
    }

    // Pattern 2: data_file_id = 123 (for single file deletion)
    let eq_patterns = [
        " data_file_id =",
        "data_file_id =",
        " delete_file_id =",
        "delete_file_id =",
    ];
    for eq_pattern in &eq_patterns {
        if let Some(eq_idx) = lower_after.find(eq_pattern) {
            let after_eq = &after_where[eq_idx + eq_pattern.len()..].trim_start();
            // Extract until comma or WHERE or semicolon
            let mut end = after_eq.len();
            for (i, ch) in after_eq.chars().enumerate() {
                if ch == ',' || ch == ';' || ch == ')' {
                    end = i;
                    break;
                }
            }
            let id_str = after_eq[..end].trim().trim_matches('\'').trim_matches('"');
            if let Ok(id) = id_str.parse::<u64>() {
                ids.push(id);
                return ids;
            }
        }
    }

    ids
}

#[cfg(test)]
mod tests {
    use super::{
        file_ids_from_where_sql, inlined_data_table_ids_from_ctid_sql, parse_insert_rows_map,
        parse_multi_statement_batch, resolve_comparison_u64, row_map_bool, row_map_optional_bool,
        row_map_string, row_map_u64,
    };
    use rocklake_sql::ParamValues;

    #[test]
    fn parses_multiline_ducklake_gc_delete() {
        assert_eq!(
            file_ids_from_where_sql(
                "DELETE FROM ducklake_data_file\nWHERE data_file_id IN (1, 2);"
            ),
            vec![1, 2]
        );
    }

    #[test]
    fn parses_quoted_inlined_data_ctids() {
        assert_eq!(
            inlined_data_table_ids_from_ctid_sql(
                "DELETE FROM ducklake_inlined_data_tables WHERE ctid IN ('(3,3)', '(3,4)')"
            ),
            vec![(3, 3), (3, 4)]
        );
    }

    #[test]
    fn catalog_literal_table_filters_are_not_ignored() {
        let params = ParamValues::default();
        for relation in ["ducklake_delete_file", "ducklake_table_stats"] {
            let sql = format!(
                "SELECT * FROM \"public\".{relation} WHERE table_id=3 AND begin_snapshot <= 4"
            );
            assert_eq!(resolve_comparison_u64(&sql, "table_id", &params), Some(3));
            assert_eq!(
                resolve_comparison_u64(&sql, "begin_snapshot", &params),
                Some(4)
            );
        }
    }

    #[test]
    fn parses_ducklake_v11_inlined_snapshot_filters() {
        assert_eq!(
            resolve_comparison_u64(
                "SELECT id FROM ducklake_inlined_data_3_4 WHERE _ducklake_begin_snapshot <= 8",
                "_ducklake_begin_snapshot",
                &ParamValues::default(),
            ),
            Some(8)
        );
    }

    #[test]
    fn parser_batch_keeps_quoted_semicolons_in_one_statement() {
        let statements = parse_multi_statement_batch("SELECT 'delete; ducklake'; SELECT 1")
            .expect("two SQL statements");
        assert_eq!(statements.len(), 2);
        assert!(statements[0].contains("delete; ducklake"));
    }

    #[test]
    fn maps_positional_ducklake_view_rows_by_catalog_column_order() {
        let rows = parse_insert_rows_map(
            "INSERT INTO ducklake_view VALUES (9, '739b25e2-02e0-43e7-9bd0-0d31db9866e8', 11, NULL, 3, 'event_view', 'duckdb', 'SELECT 1', 'event_id')",
            &ParamValues::default(),
        )
        .unwrap();
        assert_eq!(row_map_u64(&rows[0], "schema_id"), Some(3));
        assert_eq!(
            row_map_string(&rows[0], "view_name").as_deref(),
            Some("event_view")
        );
        assert_eq!(
            row_map_string(&rows[0], "view_uuid").as_deref(),
            Some("739b25e2-02e0-43e7-9bd0-0d31db9866e8")
        );
        assert_eq!(row_map_string(&rows[0], "sql").as_deref(), Some("SELECT 1"));
    }

    #[test]
    fn parses_ducklake_v11_exactness_fields_by_name() {
        let params = ParamValues::default();
        let rows = parse_insert_rows_map(
            "INSERT INTO ducklake_file_column_stats (data_file_id, table_id, column_id, min_is_exact, max_is_exact) VALUES (3, 1, 2, false, true)",
            &params,
        )
        .unwrap();
        assert_eq!(row_map_bool(&rows[0], "min_is_exact"), Some(false));
        assert_eq!(row_map_bool(&rows[0], "max_is_exact"), Some(true));

        let rows = parse_insert_rows_map(
            "INSERT INTO ducklake_table_column_stats (table_id, column_id, min_is_exact, max_is_exact) VALUES (1, 2, true, NULL)",
            &params,
        )
        .unwrap();
        assert_eq!(
            row_map_optional_bool(&rows[0], "min_is_exact"),
            Some(Some(true))
        );
        assert_eq!(row_map_optional_bool(&rows[0], "max_is_exact"), Some(None));
    }

    #[test]
    fn parses_schema_qualified_positional_statistics_rows() {
        let params = ParamValues::default();
        let table_rows = parse_insert_rows_map(
            "INSERT INTO \"public\".ducklake_table_column_stats VALUES (1, 2, false, NULL, '1', '12', NULL), (1, 3, false, NULL, 'a', 'z', NULL)",
            &params,
        )
        .unwrap();
        assert_eq!(row_map_u64(&table_rows[0], "column_id"), Some(2));
        assert_eq!(row_map_u64(&table_rows[1], "column_id"), Some(3));

        let file_rows = parse_insert_rows_map(
            "INSERT INTO \"public\".ducklake_file_column_stats VALUES (4, 1, 2, 8, 12, 0, '1', '12', false, NULL), (4, 1, 3, 8, 12, 0, 'a', 'z', false, NULL)",
            &params,
        )
        .unwrap();
        assert_eq!(row_map_u64(&file_rows[0], "column_id"), Some(2));
        assert_eq!(row_map_u64(&file_rows[1], "column_id"), Some(3));

        let table_rows = parse_insert_rows_map(
            "INSERT INTO \"public\".ducklake_table_column_stats VALUES (1, 2, false, NULL, '1', '12', NULL, true, false)",
            &params,
        )
        .unwrap();
        assert_eq!(
            row_map_optional_bool(&table_rows[0], "min_is_exact"),
            Some(Some(true))
        );

        let file_rows = parse_insert_rows_map(
            "INSERT INTO \"public\".ducklake_file_column_stats VALUES (4, 1, 2, 8, 12, 0, '1', '12', false, NULL, true, false)",
            &params,
        )
        .unwrap();
        assert_eq!(row_map_bool(&file_rows[0], "min_is_exact"), Some(true));
    }

    #[test]
    fn parses_positional_ducklake_partition_columns() {
        let rows = parse_insert_rows_map(
            "INSERT INTO ducklake_partition_column VALUES (4, 3, 0, 1, 'epoch_day')",
            &ParamValues::default(),
        )
        .unwrap();
        assert_eq!(row_map_u64(&rows[0], "partition_id"), Some(4));
        assert_eq!(row_map_u64(&rows[0], "table_id"), Some(3));
        assert_eq!(row_map_u64(&rows[0], "partition_key_index"), Some(0));
        assert_eq!(row_map_u64(&rows[0], "column_id"), Some(1));
        assert_eq!(
            row_map_string(&rows[0], "transform").as_deref(),
            Some("epoch_day")
        );
    }

    #[test]
    fn parses_positional_ducklake_file_partition_values() {
        let rows = parse_insert_rows_map(
            "INSERT INTO ducklake_file_partition_value VALUES (7, 3, 0, '-1')",
            &ParamValues::default(),
        )
        .unwrap();
        assert_eq!(row_map_u64(&rows[0], "data_file_id"), Some(7));
        assert_eq!(row_map_u64(&rows[0], "table_id"), Some(3));
        assert_eq!(row_map_u64(&rows[0], "partition_key_index"), Some(0));
        assert_eq!(
            row_map_string(&rows[0], "partition_value").as_deref(),
            Some("-1")
        );
    }
}

fn literal_insert_rows(sql: &str) -> Vec<Vec<Option<String>>> {
    let Some(values_idx) = sql.to_lowercase().find("values") else {
        return Vec::new();
    };
    let after_values = sql[values_idx + "values".len()..].trim_start();
    let mut rows = Vec::new();
    let mut depth = 0i32;
    let mut in_quote = false;
    let mut row_start = None;
    let mut chars = after_values.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch == '\'' {
            if in_quote && chars.peek().map(|(_, next)| *next == '\'').unwrap_or(false) {
                chars.next();
                continue;
            }
            in_quote = !in_quote;
        } else if !in_quote {
            match ch {
                '(' => {
                    if depth == 0 {
                        row_start = Some(idx + 1);
                    }
                    depth += 1;
                }
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(start) = row_start.take() {
                            rows.push(split_literal_values(&after_values[start..idx]));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    rows
}

fn split_literal_values(values: &str) -> Vec<Option<String>> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_quote = false;
    let mut depth = 0i32;
    let mut chars = values.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch == '\'' {
            if in_quote && chars.peek().map(|(_, next)| *next == '\'').unwrap_or(false) {
                chars.next();
                continue;
            }
            in_quote = !in_quote;
        } else if !in_quote {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(normalize_literal(&values[start..idx]));
                    start = idx + 1;
                }
                _ => {}
            }
        }
    }
    parts.push(normalize_literal(&values[start..]));
    parts
}

fn resolve_expr(expr: &sqlparser::ast::Expr, params: &ParamValues) -> Option<String> {
    match expr {
        sqlparser::ast::Expr::Value(val_with_span) => match &val_with_span.value {
            sqlparser::ast::Value::Number(n, _) => Some(n.clone()),
            sqlparser::ast::Value::SingleQuotedString(s)
            | sqlparser::ast::Value::DoubleQuotedString(s) => Some(s.clone()),
            sqlparser::ast::Value::Boolean(b) => Some(b.to_string()),
            sqlparser::ast::Value::Null => None,
            sqlparser::ast::Value::Placeholder(p) => {
                let s = p.strip_prefix('$').unwrap_or(p.as_str());
                if let Ok(idx) = s.parse::<usize>() {
                    if idx > 0 {
                        return params.get_optional_string(idx - 1);
                    }
                }
                None
            }
            _ => Some(val_with_span.value.to_string()),
        },
        sqlparser::ast::Expr::Collate { expr, .. } => resolve_expr(expr, params),
        sqlparser::ast::Expr::Cast { expr, .. } => resolve_expr(expr, params),
        sqlparser::ast::Expr::Identifier(ident) => Some(ident.value.clone()),
        _ => Some(expr.to_string()),
    }
}

fn update_table_column_stats(
    sql: &str,
    params: &ParamValues,
    has_exactness_fields: bool,
) -> Result<Vec<BufferedOp>, RockLakeError> {
    let statements = Parser::parse_sql(&PostgreSqlDialect {}, sql)
        .map_err(|error| RockLakeError::Unsupported(error.to_string()))?;
    let Some(statement) = statements.first() else {
        return Err(RockLakeError::Unsupported("empty SQL statement".into()));
    };
    let (selection, assignments, query) = match statement {
        Statement::Update {
            table,
            selection: Some(selection),
            assignments,
            ..
        } if is_table_column_stats_target(table) => (selection, assignments, None),
        Statement::Query(query) => {
            let SetExpr::Update(Statement::Update {
                table,
                selection: Some(selection),
                assignments,
                ..
            }) = query.body.as_ref()
            else {
                return Err(RockLakeError::Unsupported(
                    "DuckLake table-column stats updates require a row filter".into(),
                ));
            };
            if !is_table_column_stats_target(table) {
                return Err(RockLakeError::Unsupported(
                    "DuckLake table-column stats update targets an unsupported table".into(),
                ));
            }
            (selection, assignments, Some(query.as_ref()))
        }
        _ => {
            return Err(RockLakeError::Unsupported(
                "DuckLake table-column stats updates require a row filter".into(),
            ));
        }
    };

    fn update_rows(
        query: Option<&sqlparser::ast::Query>,
        params: &ParamValues,
    ) -> Result<Vec<std::collections::HashMap<String, Option<String>>>, RockLakeError> {
        let Some(query) = query else {
            return Ok(vec![std::collections::HashMap::new()]);
        };
        let Some(cte) = query.with.as_ref().and_then(|with| {
            with.cte_tables
                .iter()
                .find(|cte| cte.alias.name.value.eq_ignore_ascii_case("new_values"))
        }) else {
            return Err(RockLakeError::Unsupported(
                "DuckLake table-column stats updates require the new_values CTE".into(),
            ));
        };
        let (SetExpr::Values(values), columns) = (
            cte.query.body.as_ref(),
            cte.alias
                .columns
                .iter()
                .map(|column| column.name.value.to_ascii_lowercase())
                .collect::<Vec<_>>(),
        ) else {
            return Err(RockLakeError::Unsupported(
                "DuckLake table-column stats updates require literal CTE rows".into(),
            ));
        };
        if columns.is_empty() || values.rows.iter().any(|row| row.len() != columns.len()) {
            return Err(RockLakeError::Unsupported(
                "DuckLake table-column stats CTE has an invalid row shape".into(),
            ));
        }
        let rows = values
            .rows
            .iter()
            .map(|values| {
                columns
                    .iter()
                    .cloned()
                    .zip(values.iter().map(|value| resolve_expr(value, params)))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .collect::<Vec<_>>();
        if rows.iter().any(|row| row.len() != columns.len()) {
            return Err(RockLakeError::Unsupported(
                "DuckLake table-column stats CTE has duplicate column names".into(),
            ));
        }
        Ok(rows)
    }

    fn resolve_update_value(
        expr: &Expr,
        params: &ParamValues,
        row: &std::collections::HashMap<String, Option<String>>,
    ) -> Option<String> {
        let name = match expr {
            Expr::Identifier(identifier) => Some(&identifier.value),
            Expr::CompoundIdentifier(identifiers) => identifiers.last().map(|id| &id.value),
            Expr::Cast { expr, .. } => return resolve_update_value(expr, params, row),
            _ => None,
        };
        if let Some(name) = name {
            if let Some(value) = row.get(&name.to_ascii_lowercase()) {
                return value.clone();
            }
        }
        resolve_expr(expr, params)
    }

    fn column_name(expr: &Expr) -> Option<String> {
        let name = expr.to_string();
        Some(
            name.rsplit('.')
                .next()?
                .trim_matches('"')
                .to_ascii_lowercase(),
        )
    }

    fn collect_id_filters(
        expr: &Expr,
        params: &ParamValues,
        row: &std::collections::HashMap<String, Option<String>>,
        filters: &mut std::collections::HashMap<String, u64>,
    ) -> bool {
        match expr {
            Expr::Nested(expr) => collect_id_filters(expr, params, row, filters),
            Expr::BinaryOp {
                left,
                op: BinaryOperator::And,
                right,
            } => {
                collect_id_filters(left, params, row, filters)
                    && collect_id_filters(right, params, row, filters)
            }
            Expr::BinaryOp {
                left,
                op: BinaryOperator::Eq,
                right,
            } => {
                let (column, value) = if column_name(left).as_deref() == Some("table_id") {
                    ("table_id", resolve_update_value(right, params, row))
                } else if column_name(right).as_deref() == Some("table_id") {
                    ("table_id", resolve_update_value(left, params, row))
                } else if column_name(left).as_deref() == Some("column_id") {
                    ("column_id", resolve_update_value(right, params, row))
                } else if column_name(right).as_deref() == Some("column_id") {
                    ("column_id", resolve_update_value(left, params, row))
                } else {
                    return false;
                };
                let Some(value) = value.and_then(|value| value.parse::<u64>().ok()) else {
                    return false;
                };
                filters.insert(column.to_string(), value).is_none()
            }
            _ => false,
        }
    }

    let mut ops = Vec::new();
    for row in update_rows(query, params)? {
        let mut filters = std::collections::HashMap::new();
        if !collect_id_filters(selection, params, &row, &mut filters) || filters.len() != 2 {
            return Err(RockLakeError::Unsupported(
                "DuckLake table-column stats updates must filter one table_id and column_id".into(),
            ));
        }
        let mut values = std::collections::HashMap::new();
        for assignment in assignments {
            let column = assignment
                .target
                .to_string()
                .rsplit('.')
                .next()
                .unwrap_or_default()
                .trim_matches('"')
                .to_ascii_lowercase();
            if !matches!(
                column.as_str(),
                "contains_null"
                    | "contains_nan"
                    | "min_value"
                    | "max_value"
                    | "extra_stats"
                    | "min_is_exact"
                    | "max_is_exact"
            ) || values
                .insert(
                    column,
                    resolve_update_value(&assignment.value, params, &row),
                )
                .is_some()
            {
                return Err(RockLakeError::Unsupported(
                    "unsupported DuckLake table-column stats update".into(),
                ));
            }
        }
        if values.len() != if has_exactness_fields { 7 } else { 5 } {
            return Err(RockLakeError::Unsupported(
                "DuckLake table-column stats updates must set all stats fields".into(),
            ));
        }

        let required = |column: &str| {
            values
                .get(column)
                .cloned()
                .ok_or_else(|| RockLakeError::MissingParam {
                    name: column.to_string(),
                })
        };
        let parse_bool = |column: &str, value: Option<String>| {
            let Some(value) = value else {
                return Err(RockLakeError::ValueDecode(format!(
                    "{column} cannot be NULL"
                )));
            };
            match value.to_ascii_lowercase().as_str() {
                "true" | "t" | "1" => Ok(true),
                "false" | "f" | "0" => Ok(false),
                _ => Err(RockLakeError::ValueDecode(format!(
                    "invalid boolean for {column}"
                ))),
            }
        };
        let parse_nullable_bool = |column: &str, value: Option<String>| {
            value
                .map(|value| parse_bool(column, Some(value)))
                .transpose()
        };
        ops.push(BufferedOp::SetTableColumnStats {
            table_id: filters["table_id"],
            column_id: filters["column_id"],
            contains_null: parse_nullable_bool("contains_null", required("contains_null")?)?,
            contains_nan: parse_nullable_bool("contains_nan", required("contains_nan")?)?,
            min_value: required("min_value")?,
            max_value: required("max_value")?,
            extra_stats: required("extra_stats")?,
            min_is_exact: if has_exactness_fields {
                parse_nullable_bool("min_is_exact", required("min_is_exact")?)?
            } else {
                None
            },
            max_is_exact: if has_exactness_fields {
                parse_nullable_bool("max_is_exact", required("max_is_exact")?)?
            } else {
                None
            },
        });
    }
    Ok(ops)
}

fn is_table_column_stats_target(table: &sqlparser::ast::TableWithJoins) -> bool {
    let sqlparser::ast::TableFactor::Table { name, .. } = &table.relation else {
        return false;
    };
    normalize_table_ref(&name.to_string()).eq_ignore_ascii_case("ducklake_table_column_stats")
}

fn parse_insert_rows_map(
    sql: &str,
    params: &ParamValues,
) -> Option<Vec<std::collections::HashMap<String, Option<String>>>> {
    use sqlparser::ast::{SetExpr, Statement, Values};
    let dialect = PostgreSqlDialect {};
    let statements = Parser::parse_sql(&dialect, sql).ok()?;
    let Statement::Insert(insert) = statements.first()? else {
        return None;
    };

    let table_name = match &insert.table {
        sqlparser::ast::TableObject::TableName(name) => name.to_string().to_lowercase(),
        _ => String::new(),
    };
    let table_name_normalized = normalize_table_ref(&table_name);

    let mut cols: Vec<String> = insert
        .columns
        .iter()
        .map(|c| c.value.to_lowercase())
        .collect();

    if cols.is_empty() {
        if table_name_normalized == "ducklake_data_file"
            && insert.source.as_ref().is_some_and(|source| matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 16)))
        {
            cols = crate::schema_registry::data_file_schema().iter()
                .map(|field| field.name().to_string()).collect();
        } else if table_name_normalized == "ducklake_data_file"
            && insert.source.as_ref().is_some_and(|source| matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 17)))
        {
            cols = crate::schema_registry::data_file_schema_v11().iter()
                .map(|field| field.name().to_string()).collect();
        } else if table_name_normalized == "ducklake_delete_file"
            && insert.source.as_ref().is_some_and(|source| matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 13)))
        {
            cols = crate::schema_registry::delete_file_schema().iter()
                .map(|field| field.name().to_string()).collect();
        } else if table_name_normalized == "ducklake_view"
            && insert.source.as_ref().is_some_and(|source| matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 9)))
        {
            cols = crate::schema_registry::view_schema()
                .iter()
                .map(|field| field.name().to_string())
                .collect();
        } else if table_name_normalized == "ducklake_table_column_stats" {
            let v11 = insert.source.as_ref().is_some_and(|source| {
                matches!(&*source.body, SetExpr::Values(values) if values.rows.first().is_some_and(|row| row.len() == 9))
            });
            let schema = if v11 {
                crate::schema_registry::table_column_stats_schema_v11()
            } else {
                crate::schema_registry::table_column_stats_schema()
            };
            cols = schema.iter().map(|field| field.name().to_string()).collect();
        } else if table_name_normalized == "ducklake_file_column_stats" {
            let v11 = insert.source.as_ref().is_some_and(|source| {
                matches!(&*source.body, SetExpr::Values(values) if values.rows.first().is_some_and(|row| row.len() == 12))
            });
            let schema = if v11 {
                crate::schema_registry::file_column_stats_schema_v11()
            } else {
                crate::schema_registry::file_column_stats_schema()
            };
            cols = schema.iter().map(|field| field.name().to_string()).collect();
        } else if table_name_normalized == "ducklake_delete_file"
            && insert.source.as_ref().is_some_and(|source| matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 14)))
        {
            cols = crate::schema_registry::delete_file_schema_v11().iter()
                .map(|field| field.name().to_string()).collect();
        } else if table_name_normalized == "ducklake_column_mapping"
            && insert.source.as_ref().is_some_and(|source| {
                matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 3))
            })
        {
            cols = crate::schema_registry::column_mapping_schema()
                .iter()
                .map(|field| field.name().to_string())
                .collect();
        } else if table_name_normalized == "ducklake_name_mapping"
            && insert.source.as_ref().is_some_and(|source| {
                matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 6))
            })
        {
            cols = crate::schema_registry::name_mapping_schema()
                .iter()
                .map(|field| field.name().to_string())
                .collect();
        } else if table_name_normalized == "ducklake_column_mapping" {
            cols = vec!["table_id".to_string(), "mapping_type".to_string()];
        } else if table_name_normalized == "ducklake_name_mapping" {
            cols = vec!["column_id".to_string(), "name".to_string()];
        } else if table_name_normalized == "ducklake_partition_info" {
            cols = vec![
                "partition_id".to_string(),
                "table_id".to_string(),
                "begin_snapshot".to_string(),
                "end_snapshot".to_string(),
            ];
        } else if table_name_normalized == "ducklake_partition_column"
            && insert.source.as_ref().is_some_and(|source| {
                matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 5))
            })
        {
            cols = crate::schema_registry::partition_column_schema()
                .iter()
                .map(|field| field.name().to_string())
                .collect();
        } else if table_name_normalized == "ducklake_file_partition_value"
            && insert.source.as_ref().is_some_and(|source| {
                matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 4))
            })
        {
            cols = crate::schema_registry::file_partition_value_schema()
                .iter()
                .map(|field| field.name().to_string())
                .collect();
        } else if table_name_normalized == "ducklake_sort_info" {
            cols = vec![
                "sort_id".to_string(),
                "table_id".to_string(),
                "begin_snapshot".to_string(),
                "end_snapshot".to_string(),
            ];
        } else if table_name_normalized == "ducklake_sort_expression"
            && insert.source.as_ref().is_some_and(|source| {
                matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 7))
            })
        {
            cols = crate::schema_registry::sort_expression_schema()
                .iter()
                .map(|field| field.name().to_string())
                .collect();
        } else if table_name_normalized == "ducklake_files_scheduled_for_deletion" {
            cols = vec![
                "data_file_id".to_string(),
                "path".to_string(),
                "path_is_relative".to_string(),
                "schedule_start".to_string(),
                "file_type".to_string(),
            ];
        } else if table_name_normalized.contains("ducklake_table") {
            cols = vec![
                "table_id".to_string(),
                "table_uuid".to_string(),
                "begin_snapshot".to_string(),
                "end_snapshot".to_string(),
                "schema_id".to_string(),
                "table_name".to_string(),
                "path".to_string(),
                "path_is_relative".to_string(),
            ];
        } else if table_name_normalized.contains("ducklake_column") {
            cols = vec![
                "column_id".to_string(),
                "begin_snapshot".to_string(),
                "end_snapshot".to_string(),
                "table_id".to_string(),
                "column_order".to_string(),
                "column_name".to_string(),
                "column_type".to_string(),
                "initial_default".to_string(),
                "default_value".to_string(),
                "nulls_allowed".to_string(),
                "parent_column".to_string(),
                "default_value_type".to_string(),
                "default_value_dialect".to_string(),
            ];
        } else if table_name_normalized == "ducklake_schema"
            && insert.source.as_ref().is_some_and(|source| matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 7)))
        {
            cols = vec![
                "schema_id".to_string(),
                "schema_uuid".to_string(),
                "begin_snapshot".to_string(),
                "end_snapshot".to_string(),
                "schema_name".to_string(),
                "path".to_string(),
                "path_is_relative".to_string(),
            ];
        } else if table_name_normalized == "ducklake_schema"
            && insert.source.as_ref().is_some_and(|source| matches!(&*source.body, SetExpr::Values(values) if values.rows.iter().all(|row| row.len() == 8)))
        {
            cols = crate::schema_registry::schema_schema_v11()
                .iter()
                .map(|field| field.name().to_string())
                .collect();
        }
    }

    let source = insert.source.as_ref()?;
    let SetExpr::Values(Values { rows, .. }) = &*source.body else {
        return None;
    };

    let mut result_rows = Vec::new();
    for row in rows {
        let mut row_map = std::collections::HashMap::new();
        for (i, expr) in row.iter().enumerate() {
            if i < cols.len() {
                let val = resolve_expr(expr, params);
                row_map.insert(cols[i].clone(), val);
            }
        }
        result_rows.push(row_map);
    }
    Some(result_rows)
}

pub(super) fn row_map_string(
    row: &std::collections::HashMap<String, Option<String>>,
    key: &str,
) -> Option<String> {
    row.get(key).and_then(|value| value.clone())
}

pub(super) fn row_map_u64(
    row: &std::collections::HashMap<String, Option<String>>,
    key: &str,
) -> Option<u64> {
    row.get(key)
        .and_then(|value| value.as_ref().and_then(|text| text.parse::<u64>().ok()))
}

pub(super) fn row_map_bool(
    row: &std::collections::HashMap<String, Option<String>>,
    key: &str,
) -> Option<bool> {
    row.get(key).and_then(|value| {
        value
            .as_ref()
            .and_then(|text| match text.to_ascii_lowercase().as_str() {
                "true" | "t" | "1" => Some(true),
                "false" | "f" | "0" => Some(false),
                _ => None,
            })
    })
}

fn row_map_optional_bool(
    row: &std::collections::HashMap<String, Option<String>>,
    key: &str,
) -> Option<Option<bool>> {
    row.get(key).map(|value| {
        value
            .as_ref()
            .and_then(|text| match text.to_ascii_lowercase().as_str() {
                "true" | "t" | "1" => Some(true),
                "false" | "f" | "0" => Some(false),
                _ => None,
            })
    })
}

fn required_row_u64_value(
    row: &std::collections::HashMap<String, Option<String>>,
    names: &[&str],
) -> Result<u64, RockLakeError> {
    names
        .iter()
        .find_map(|name| row_map_u64(row, name))
        .ok_or_else(|| RockLakeError::MissingParam {
            name: names[0].to_string(),
        })
}

fn required_row_string_value(
    row: &std::collections::HashMap<String, Option<String>>,
    names: &[&str],
) -> Result<String, RockLakeError> {
    names
        .iter()
        .find_map(|name| row_map_string(row, name).filter(|value| !value.is_empty()))
        .ok_or_else(|| RockLakeError::MissingParam {
            name: names[0].to_string(),
        })
}

fn optional_row_i64_value(
    row: &std::collections::HashMap<String, Option<String>>,
    name: &str,
) -> Result<Option<i64>, RockLakeError> {
    match row.get(name).and_then(|value| value.as_ref()) {
        None => Ok(None),
        Some(value) => value
            .parse::<i64>()
            .map(Some)
            .map_err(|_| RockLakeError::MissingParam {
                name: name.to_string(),
            }),
    }
}

fn normalize_literal(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        return None;
    }
    if trimmed.len() >= 2 && trimmed.starts_with('\'') && trimmed.ends_with('\'') {
        return Some(trimmed[1..trimmed.len() - 1].replace("''", "'"));
    }
    // Unwrap CAST('literal' AS type) or CAST(literal AS type) — recurse on inner
    if let Some(inner) = cast_inner_literal(trimmed) {
        return normalize_literal(inner);
    }
    Some(trimmed.to_string())
}

/// If `s` looks like `CAST(<literal> AS <type>)`, return the inner literal slice.
fn cast_inner_literal(s: &str) -> Option<&str> {
    let lower = s.to_ascii_lowercase();
    // Must start with "cast("
    let after_cast = lower.strip_prefix("cast(")?;
    let offset = s.len() - after_cast.len(); // byte offset of the content after "cast("
    let inner = s[offset..].trim_start();
    let trim_skip = s[offset..].len() - inner.len();
    let inner_start = offset + trim_skip;
    if s.as_bytes().get(inner_start).copied() != Some(b'\'') {
        return None;
    }
    // Scan for the matching closing quote in the original string
    let bytes = s.as_bytes();
    let mut i = inner_start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if bytes.get(i + 1).copied() == Some(b'\'') {
                i += 2; // escaped single-quote, skip both
            } else {
                return Some(&s[inner_start..=i]); // return including quotes
            }
        } else {
            i += 1;
        }
    }
    None
}

fn literal_string(values: &[Option<String>], index: usize) -> Option<String> {
    values.get(index).cloned().flatten()
}

fn literal_u64(values: &[Option<String>], index: usize) -> Option<u64> {
    literal_string(values, index).and_then(|value| value.parse::<u64>().ok())
}

fn literal_bool(values: &[Option<String>], index: usize) -> Option<bool> {
    literal_string(values, index).and_then(|value| match value.to_ascii_lowercase().as_str() {
        "true" | "t" | "1" => Some(true),
        "false" | "f" | "0" => Some(false),
        _ => None,
    })
}

fn literal_assignment_u64(sql: &str, column: &str) -> Option<u64> {
    let lower = sql.to_ascii_lowercase();
    let column_lower = column.to_ascii_lowercase();
    let idx = lower.find(&column_lower)?;
    let after_column = &sql[idx + column.len()..];
    let equals_idx = after_column.find('=')?;
    parse_leading_u64(&after_column[equals_idx + 1..])
}

fn literal_comparison_u64(sql: &str, column: &str) -> Option<u64> {
    let lower = sql.to_ascii_lowercase();
    let where_idx = lower.find("where")?;
    let where_clause = &lower[where_idx..];
    let column_lower = column.to_ascii_lowercase();
    let idx = where_clause.find(&column_lower)?;
    let after_column = &sql[where_idx + idx + column.len()..];
    let comp_idx = after_column.find(['=', '<', '>'])?;
    let mut op_len = 0;
    for c in after_column[comp_idx..].chars() {
        if matches!(c, '=' | '<' | '>') {
            op_len += 1;
        } else {
            break;
        }
    }
    parse_leading_u64(&after_column[comp_idx + op_len..])
}

#[derive(Debug, Clone)]
struct WhereClauseId {
    column: String,
    value: u64,
}

fn extract_id_from_where_clause(sql: &str) -> Option<WhereClauseId> {
    let lower = sql.to_ascii_lowercase();
    let where_idx = lower.find("where")?;
    let where_clause = &lower[where_idx..];

    // Look for identifiers: table_id, column_id, object_id, view_id, macro_id, schema_id
    for ident in &[
        "data_file_id",
        "delete_file_id",
        "table_id",
        "column_id",
        "object_id",
        "view_id",
        "macro_id",
        "schema_id",
    ] {
        if let Some(idx) = where_clause.find(ident) {
            let after_ident = &where_clause[idx + ident.len()..];

            // Check for IN (<num>) format
            if let Some(in_idx) = after_ident.find("in") {
                let rest = after_ident[in_idx + 2..].trim_start();
                if rest.starts_with('(') {
                    let digits: String = rest
                        .chars()
                        .skip(1)
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(num) = digits.parse::<u64>() {
                        return Some(WhereClauseId {
                            column: ident.to_string(),
                            value: num,
                        });
                    }
                }
            }

            // Check for = <num> format
            if let Some(eq_idx) = after_ident.find('=') {
                let rest = after_ident[eq_idx + 1..].trim_start();
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(num) = digits.parse::<u64>() {
                    return Some(WhereClauseId {
                        column: ident.to_string(),
                        value: num,
                    });
                }
            }
        }
    }
    None
}

fn ducklake_dropped_column_ids(sql: &str) -> Option<(u64, u64)> {
    let lower = sql.to_ascii_lowercase();
    let with_idx = lower.find("with dropped_cols")?;
    let values_idx = lower[with_idx..].find("values")? + with_idx;
    let after_values = &lower[values_idx + "values".len()..];
    let open_idx = after_values.find('(')?;
    let tuple = &after_values[open_idx + 1..];

    let mut numbers = tuple
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok());

    Some((numbers.next()?, numbers.next()?))
}

fn placeholder_comparison_index(sql: &str, column: &str) -> Option<usize> {
    let lower = sql.to_ascii_lowercase();
    let where_idx = lower.find("where")?;
    let where_clause = &lower[where_idx..];
    let column_lower = column.to_ascii_lowercase();
    let idx = where_clause.find(&column_lower)?;
    let after_column = &sql[where_idx + idx + column.len()..];
    let comp_idx = after_column.find(['=', '<', '>'])?;
    let mut op_len = 0;
    for c in after_column[comp_idx..].chars() {
        if matches!(c, '=' | '<' | '>') {
            op_len += 1;
        } else {
            break;
        }
    }
    let rest = after_column[comp_idx + op_len..].trim_start();
    if rest.starts_with('$') {
        let digits: String = rest
            .chars()
            .skip(1)
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let idx = digits.parse::<usize>().ok()?;
        if idx > 0 {
            return Some(idx - 1);
        }
    }
    None
}

fn resolve_comparison_u64(sql: &str, column: &str, params: &ParamValues) -> Option<u64> {
    if let Some(idx) = placeholder_comparison_index(sql, column) {
        if let Ok(val) = params.get_u64(idx) {
            return Some(val);
        }
    }
    literal_comparison_u64(sql, column)
}

fn resolve_comparison_string(sql: &str, column: &str, params: &ParamValues) -> Option<String> {
    let statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).ok()?;
    let selection = match statements.first()? {
        Statement::Query(query) => {
            let SetExpr::Select(select) = query.body.as_ref() else {
                return None;
            };
            select.selection.as_ref()?
        }
        Statement::Update {
            selection: Some(selection),
            ..
        } => selection,
        Statement::Delete(delete) => delete.selection.as_ref()?,
        _ => return None,
    };
    let mut value = None;
    let _: std::ops::ControlFlow<()> = visit_expressions(selection, |expr| {
        if let Expr::BinaryOp {
            left,
            op: BinaryOperator::Eq,
            right,
        } = expr
        {
            if expression_matches_column(left, column) {
                value = resolve_expr(right, params);
            } else if expression_matches_column(right, column) {
                value = resolve_expr(left, params);
            }
        }
        std::ops::ControlFlow::Continue(())
    });
    value
}

fn resolve_assignment_string(sql: &str, column: &str, params: &ParamValues) -> Option<String> {
    let statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).ok()?;
    let Statement::Update { assignments, .. } = statements.first()? else {
        return None;
    };
    assignments.iter().find_map(|assignment| {
        let target = assignment
            .target
            .to_string()
            .split('.')
            .next_back()?
            .trim_matches('"')
            .to_string();
        target
            .eq_ignore_ascii_case(column)
            .then(|| resolve_expr(&assignment.value, params))
            .flatten()
    })
}

fn expression_matches_column(expr: &Expr, column: &str) -> bool {
    match expr {
        Expr::Identifier(id) => id.value.eq_ignore_ascii_case(column),
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .is_some_and(|id| id.value.eq_ignore_ascii_case(column)),
        Expr::Nested(expr) => expression_matches_column(expr, column),
        _ => false,
    }
}

fn updates_column(sql: &str, column: &str) -> bool {
    let Ok(statements) = Parser::parse_sql(&PostgreSqlDialect {}, sql) else {
        return false;
    };
    let Some(Statement::Update { assignments, .. }) = statements.first() else {
        return false;
    };
    assignments.iter().any(|assignment| {
        assignment
            .target
            .to_string()
            .split('.')
            .next_back()
            .unwrap_or_default()
            .trim_matches('"')
            .eq_ignore_ascii_case(column)
    })
}

#[derive(Clone, Copy)]
enum InlinedRowSelection {
    All,
    Visible,
    Open,
    Ended,
}

fn is_end_snapshot_column(expr: &Expr, stable_1_1: bool) -> bool {
    match expr {
        Expr::Identifier(id) => id.value.eq_ignore_ascii_case(if stable_1_1 {
            "_ducklake_end_snapshot"
        } else {
            "end_snapshot"
        }),
        Expr::CompoundIdentifier(ids) => ids.last().is_some_and(|id| {
            id.value.eq_ignore_ascii_case(if stable_1_1 {
                "_ducklake_end_snapshot"
            } else {
                "end_snapshot"
            })
        }),
        Expr::Nested(expr) => is_end_snapshot_column(expr, stable_1_1),
        _ => false,
    }
}

pub(crate) fn inlined_select_star_limit_zero(sql: &str) -> bool {
    let Ok(statements) = Parser::parse_sql(&PostgreSqlDialect {}, sql) else {
        return false;
    };
    let Some(Statement::Query(query)) = statements.first() else {
        return false;
    };
    let SetExpr::Select(select) = query.body.as_ref() else {
        return false;
    };
    query
        .limit
        .as_ref()
        .is_some_and(|limit| limit.to_string() == "0")
        && select.projection.iter().any(|item| {
            matches!(
                item,
                sqlparser::ast::SelectItem::Wildcard(_)
                    | sqlparser::ast::SelectItem::QualifiedWildcard(_, _)
            )
        })
}

fn required_end_snapshot_nullness(expr: &Expr, stable_1_1: bool) -> Option<bool> {
    match expr {
        Expr::Nested(expr) => required_end_snapshot_nullness(expr, stable_1_1),
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => required_end_snapshot_nullness(left, stable_1_1)
            .or_else(|| required_end_snapshot_nullness(right, stable_1_1)),
        Expr::IsNull(expr) if is_end_snapshot_column(expr, stable_1_1) => Some(false),
        Expr::IsNotNull(expr) if is_end_snapshot_column(expr, stable_1_1) => Some(true),
        _ => None,
    }
}

fn inlined_row_selection(sql: &str, stable_1_1: bool) -> InlinedRowSelection {
    let Ok(statements) = Parser::parse_sql(&PostgreSqlDialect {}, sql) else {
        return InlinedRowSelection::Visible;
    };
    let Some(Statement::Query(query)) = statements.first() else {
        return InlinedRowSelection::Visible;
    };
    let SetExpr::Select(select) = query.body.as_ref() else {
        return InlinedRowSelection::Visible;
    };
    let Some(selection) = &select.selection else {
        return InlinedRowSelection::All;
    };
    if let Some(is_ended) = required_end_snapshot_nullness(selection, stable_1_1) {
        return if is_ended {
            InlinedRowSelection::Ended
        } else {
            InlinedRowSelection::Open
        };
    }
    let mut references_end_snapshot = false;
    let _: std::ops::ControlFlow<()> = visit_expressions(selection, |expr| {
        references_end_snapshot |= is_end_snapshot_column(expr, stable_1_1);
        std::ops::ControlFlow::Continue(())
    });
    if references_end_snapshot {
        InlinedRowSelection::Visible
    } else {
        InlinedRowSelection::All
    }
}

fn parse_leading_u64(input: &str) -> Option<u64> {
    let trimmed = input.trim_start();
    let digits: String = trimmed
        .chars()
        .skip_while(|ch| *ch == '\'')
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits.parse::<u64>().ok()
}

fn ducklake_insert_table_name(sql: &str) -> Option<String> {
    let lower = sql.to_ascii_lowercase();
    let insert_idx = lower.find("insert into")?;
    let after_insert = sql[insert_idx + "insert into".len()..].trim_start();
    let values_idx = after_insert.to_ascii_lowercase().find(" values")?;
    let table_ref = after_insert[..values_idx]
        .trim()
        .split('(')
        .next()
        .unwrap_or(after_insert[..values_idx].trim())
        .trim();
    Some(normalize_table_ref(table_ref))
}

fn normalize_table_ref(table_ref: &str) -> String {
    table_ref
        .split('.')
        .next_back()
        .unwrap_or(table_ref)
        .trim()
        .trim_matches('"')
        .to_string()
}

fn inlined_table_name_from_sql(sql: &str) -> Option<String> {
    let lower = sql.to_ascii_lowercase();
    let start = ["ducklake_inlined_data_", "ducklake_inlined_delete_"]
        .into_iter()
        .filter_map(|prefix| lower.find(prefix))
        .min()?;
    let rest = &sql[start..];
    let end = rest
        .char_indices()
        .find_map(|(idx, ch)| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                None
            } else {
                Some(idx)
            }
        })
        .unwrap_or(rest.len());
    Some(rest[..end].trim_matches('"').to_string())
}

fn parse_inlined_delete_table_id(table_name: &str) -> Option<u64> {
    table_name
        .trim_matches('"')
        .to_ascii_lowercase()
        .strip_prefix("ducklake_inlined_delete_")?
        .parse()
        .ok()
}

#[allow(clippy::too_many_arguments)]
async fn execute_classified<'a>(
    kind: StatementKind,
    _sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
    session: &mut SessionState,
    notify_manager: &Arc<NotifyManager>,
    extension_schemas: &Arc<Vec<String>>,
    mode: AccessMode,
) -> Result<Vec<Response<'a>>, RockLakeError> {
    ensure_access(mode, &kind, _sql)?;
    tracing::debug!(statement_kind = ?kind, "executing classified SQL statement");
    match kind {
        // ─── Session / Introspection ───────────────────────────────────
        StatementKind::SelectVersion => Ok(vec![make_single_text_response(
            "version",
            Some("PostgreSQL 15.0 on x86_64-pc-linux-gnu"),
        )]),
        StatementKind::SelectVersionWithRdsCheck => {
            Ok(vec![make_version_with_rds_check_response()])
        }
        StatementKind::SelectOne => Ok(vec![make_single_int_response("?column?", Some(1))]),
        StatementKind::SelectCurrentSchema => Ok(vec![make_single_text_response(
            "current_schema",
            Some("public"),
        )]),
        StatementKind::SelectCurrentDatabase => Ok(vec![make_single_text_response(
            "current_database",
            Some("ducklake"),
        )]),
        StatementKind::SelectPgType => Ok(vec![make_pg_type_response()]),
        StatementKind::SelectPgTables => {
            let exists =
                if let Some(table_name) = resolve_comparison_string(_sql, "tablename", params) {
                    if let Some(table_id) = parse_inlined_delete_table_id(&table_name) {
                        let (reader, relations) = {
                            let catalog = store.lock().await;
                            (catalog.read_latest(), catalog.bootstrap_relation_names())
                        };
                        relations.contains(&table_name)
                            || !reader
                                .list_inlined_deletes(table_id)
                                .await
                                .map_err(RockLakeError::from)?
                                .is_empty()
                    } else {
                        false
                    }
                } else {
                    false
                };
            Ok(vec![make_pg_tables_probe_response(exists)])
        }
        StatementKind::ShowVariable(ref var) => {
            let val = get_show_value(var, session);
            Ok(vec![make_single_text_response(var, Some(&val))])
        }
        StatementKind::SetVariable(ref var, ref val) => {
            apply_set(var, val, session);
            Ok(vec![Response::Execution(Tag::new("SET"))])
        }

        // ─── Session / Connection Management (DuckDB postgres scanner) ─
        StatementKind::DiscardAll => {
            // DISCARD ALL: session cleanup when DuckDB returns a connection to
            // the pool. RockLake is stateless per connection so this is a no-op.
            Ok(vec![Response::Execution(Tag::new("DISCARD"))])
        }
        StatementKind::SelectToRegclass => {
            // to_regclass('name') — return NULL to tell DuckDB the relation
            // does not exist (RockLake has no duckdb_secrets table).
            Ok(vec![make_single_text_response("to_regclass", None)])
        }
        StatementKind::SelectExistsInfoSchema => {
            // EXISTS(SELECT 1 FROM information_schema.tables WHERE ...) — return
            // false; RockLake does not expose information_schema.
            Ok(vec![make_false_bool_response("exists")])
        }
        StatementKind::SelectPgDatabaseSize => {
            // pg_database_size(current_database()) — informational only; return 0.
            Ok(vec![make_single_int_response("pg_database_size", Some(0))])
        }
        StatementKind::PgCatalogScan => {
            if _sql.to_ascii_lowercase().contains("pg_class.xmin") {
                let reader = { store.lock().await.read_latest() };
                let snapshot_id = reader.snapshot_id().as_u64();
                let initialized = reader
                    .get_snapshot()
                    .await
                    .map_err(RockLakeError::from)?
                    .is_some();
                let ducklake_v11 = reader
                    .get_metadata(rocklake_core::keys::MetadataScope::Global, 0, "version")
                    .await
                    .map_err(RockLakeError::from)?
                    .is_some_and(|row| row.value == "1.1" || row.value == "1.1-dev1");
                return Ok(vec![make_pg_catalog_staleness_response(
                    snapshot_id,
                    initialized,
                    ducklake_v11,
                )]);
            }
            let stable_1_1 = stable_ducklake_v11(store).await?;
            if let Some(table_name) = inlined_table_name_from_sql(_sql) {
                if let Some((table_id, _)) = parse_inlined_table_ids(&table_name) {
                    let reader = { store.lock().await.read_latest() };
                    if let Some((_, columns)) = reader
                        .describe_table(table_id)
                        .await
                        .map_err(RockLakeError::from)?
                    {
                        return Ok(vec![make_pg_catalog_inlined_table_response(
                            &table_name,
                            columns,
                            _sql,
                            stable_1_1,
                        )]);
                    }
                }
            }
            let (reader, mut bootstrap_relations) = {
                let catalog = store.lock().await;
                (catalog.read_latest(), catalog.bootstrap_relation_names())
            };
            for table_id in reader
                .list_inlined_delete_table_ids()
                .await
                .map_err(RockLakeError::from)?
            {
                bootstrap_relations.insert(format!("ducklake_inlined_delete_{table_id}"));
            }
            let initialized = reader
                .get_snapshot()
                .await
                .map_err(RockLakeError::from)?
                .is_some();
            let mut responses =
                make_pg_catalog_scan_responses(_sql, initialized, &bootstrap_relations, stable_1_1);
            // The postgres scanner also issues individual pg_class queries
            // to discover a table after CREATE TABLE. Only the multi-statement
            // catalog probe returns the five result sets plus ROLLBACK.
            // The multi-statement catalog discovery query may start with
            // SELECT (DuckDB 2.0), not only BEGIN (DuckDB 1.5.x).
            // Detect its statement count instead of relying on the first token.
            let statement_count = _sql
                .split(';')
                .filter(|part| !part.trim().is_empty())
                .count();
            if statement_count == 1 {
                return Ok(vec![responses.remove(1)]);
            }
            if !_sql.trim_start().to_ascii_lowercase().starts_with("begin") {
                // The newer scanner emits five SELECT queries without an
                // explicit ROLLBACK; do not invent a sixth result set.
                responses.pop();
            }
            Ok(responses)
        }

        // ─── Transaction Control ───────────────────────────────────────
        StatementKind::Begin => {
            session.in_transaction = true;
            session.ducklake_v11_migration_in_progress = false;
            let latest_snap = { store.lock().await.read_latest().snapshot_id().as_u64() };
            session.transaction_snapshot_id = Some(latest_snap);
            Ok(vec![Response::TransactionStart(Tag::new("BEGIN"))])
        }
        StatementKind::Commit => {
            let bootstrap = std::mem::take(&mut session.bootstrap);
            let ops = session.pending_txn.take();
            session.in_transaction = false;
            session.transaction_snapshot_id = None;
            session.ducklake_v11_migration_in_progress = false;

            // If this transaction contained COPY FROM STDIN bootstrap data
            // (i.e. DuckDB ATTACH initialisation), convert those rows into
            // catalog ops — but only if the catalog is still fresh (no prior
            // snapshot).  Subsequent ATTACH on an already-populated catalog
            // must NOT re-create the initial schema/snapshot.
            let mut all_ops: Vec<BufferedOp> = Vec::new();
            if bootstrap.has_snapshot
                || !bootstrap.schemas.is_empty()
                || !bootstrap.metadata.is_empty()
            {
                let reader = { store.lock().await.read_latest() };
                let existing = reader.get_snapshot().await.map_err(RockLakeError::from)?;
                if existing.is_none() {
                    // Fresh catalog: create schemas first so next_catalog_id
                    // is correctly captured in the subsequent snapshot row.
                    for schema_row in bootstrap.schemas {
                        all_ops.push(BufferedOp::InsertSchema {
                            schema_name: schema_row.schema_name,
                            schema_uuid: None,
                            path: None,
                            path_is_relative: None,
                            parent_schema_id: None,
                        });
                    }
                    for metadata_row in bootstrap.metadata {
                        all_ops.push(BufferedOp::InsertMetadata {
                            key: metadata_row.key,
                            value: metadata_row.value,
                            scope: metadata_row.scope,
                            scope_id: metadata_row.scope_id,
                        });
                    }
                    if bootstrap.has_snapshot {
                        all_ops.push(BufferedOp::InsertSnapshot {
                            author: None,
                            message: Some("DuckDB ATTACH bootstrap".to_string()),
                        });
                    }
                } else {
                    use rocklake_core::keys::MetadataScope;
                    for metadata_row in bootstrap.metadata {
                        if metadata_row.key == "version" {
                            continue;
                        }
                        let scope = match metadata_row.scope.as_deref() {
                            Some("schema") => MetadataScope::Schema,
                            Some("table") => MetadataScope::Table,
                            _ => MetadataScope::Global,
                        };
                        let scope_id = metadata_row.scope_id.unwrap_or(0);
                        let stored = reader
                            .get_metadata(scope, scope_id, &metadata_row.key)
                            .await
                            .map_err(RockLakeError::from)?;
                        if stored.as_ref().map(|row| row.value.as_str())
                            != Some(metadata_row.value.as_str())
                        {
                            all_ops.push(BufferedOp::InsertMetadata {
                                key: metadata_row.key,
                                value: metadata_row.value,
                                scope: metadata_row.scope,
                                scope_id: metadata_row.scope_id,
                            });
                        }
                    }
                }
            }
            session.metadata_ctid_rows.clear();
            session.inlined_delete_ctid_rows.clear();
            all_ops.extend(ops);
            execute_commit(all_ops, store, notify_manager).await?;
            Ok(vec![Response::TransactionEnd(Tag::new("COMMIT"))])
        }
        StatementKind::Rollback => {
            session.pending_txn.clear();
            session.bootstrap = Default::default();
            session.metadata_ctid_rows.clear();
            session.inlined_delete_ctid_rows.clear();
            session.in_transaction = false;
            session.transaction_snapshot_id = None;
            session.ducklake_v11_migration_in_progress = false;
            Ok(vec![Response::TransactionEnd(Tag::new("ROLLBACK"))])
        }

        // ─── Read Operations ───────────────────────────────────────────
        StatementKind::SelectMaxSnapshot => {
            // F-11: clone reader out of mutex, drop lock before async I/O.
            let reader = { store.lock().await.read_latest() };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            let id = snap.map(|s| s.snapshot_id).unwrap_or(0);
            Ok(vec![make_single_int_response("max", Some(id as i64))])
        }
        // pg-trickle CDC startup: SELECT ducklake_latest_snapshot_id($1::regclass).
        // Returns the snapshot_id of the latest visible snapshot (or NULL if none).
        // This function gates the `table_changes()` call — without it pg-trickle
        // cannot register a DUCKLAKE_CHANGE_FEED and returns SQLSTATE 42883.
        StatementKind::SelectLatestSnapshotId => {
            let reader = { store.lock().await.read_latest() };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            match snap {
                Some(s) => Ok(vec![make_single_int_response(
                    "ducklake_latest_snapshot_id",
                    Some(s.snapshot_id as i64),
                )]),
                None => Ok(vec![make_single_int_response(
                    "ducklake_latest_snapshot_id",
                    None,
                )]),
            }
        }
        StatementKind::SelectLatestSnapshotInfo => {
            let snapshot_id =
                resolve_comparison_u64(_sql, "snapshot_id", params).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snapshot_id).await?;
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            let include_version = _sql.to_ascii_lowercase().contains("ducklake_metadata");
            let version = if include_version {
                reader
                    .get_metadata(rocklake_core::keys::MetadataScope::Global, 0, "version")
                    .await
                    .map_err(RockLakeError::from)?
                    .map(|row| row.value)
                    // Fresh catalogs default to DuckLake 1.1 without a stored version row.
                    .or_else(|| Some("1.1".to_string()))
            } else {
                None
            };
            Ok(vec![make_latest_snapshot_info_response(
                snap,
                include_version,
                version,
            )])
        }
        StatementKind::SelectSnapshotStatsAndChanges => {
            let reader = { store.lock().await.read_latest() };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            let stats_rows = reader
                .list_all_table_stats()
                .await
                .map_err(RockLakeError::from)?;
            let column_stats = reader
                .list_all_table_column_stats()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_snapshot_stats_changes_response(
                snap,
                stats_rows,
                column_stats,
            )])
        }
        StatementKind::SelectSchemas => {
            let snap_id = resolve_comparison_u64(_sql, "begin_snapshot", params)
                .unwrap_or_else(|| get_snapshot_param(params));
            let reader = resolve_reader(store, session, snap_id).await?;
            let mut schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
            if let Some(schema_id) = resolve_comparison_u64(_sql, "schema_id", params) {
                schemas.retain(|schema| schema.schema_id == schema_id);
            }
            if let Some(schema_name) = resolve_comparison_string(_sql, "schema_name", params) {
                schemas.retain(|schema| schema.schema_name == schema_name);
            }
            Ok(vec![make_schemas_response(
                schemas,
                _sql,
                stable_ducklake_v11(store).await?,
            )])
        }
        StatementKind::SelectTables => {
            let schema_id = resolve_comparison_u64(_sql, "schema_id", params);
            let snap_id =
                resolve_comparison_u64(_sql, "begin_snapshot", params).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let raw_tables = if let Some(schema_id) = schema_id {
                reader
                    .list_tables(schema_id)
                    .await
                    .map_err(RockLakeError::from)?
            } else {
                let schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
                let mut tables = Vec::new();
                for schema in schemas {
                    tables.extend(
                        reader
                            .list_tables(schema.schema_id)
                            .await
                            .map_err(RockLakeError::from)?,
                    );
                }
                tables
            };
            Ok(vec![make_tables_response(raw_tables)])
        }
        StatementKind::SelectColumns => {
            let table_id = resolve_comparison_u64(_sql, "table_id", params);
            let snap_id =
                resolve_comparison_u64(_sql, "begin_snapshot", params).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let columns = if let Some(table_id) = table_id {
                let result = reader
                    .describe_table(table_id)
                    .await
                    .map_err(RockLakeError::from)?;
                result.map(|(_, cols)| cols).unwrap_or_default()
            } else {
                let schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
                let mut columns = Vec::new();
                for schema in schemas {
                    for table in reader
                        .list_tables(schema.schema_id)
                        .await
                        .map_err(RockLakeError::from)?
                    {
                        if let Some((_, table_columns)) = reader
                            .describe_table(table.table_id)
                            .await
                            .map_err(RockLakeError::from)?
                        {
                            columns.extend(table_columns);
                        }
                    }
                }
                columns
            };
            Ok(vec![make_columns_response(_sql, columns)])
        }
        StatementKind::SelectExtendedDataFiles => {
            // DuckDB's extended COPY query filters data files by snapshot and
            // optionally by column statistics. A superset of live files is
            // correct: zone-map pruning affects performance, not results.
            let table_id = resolve_comparison_u64(_sql, "table_id", params)
                .or_else(|| literal_assignment_u64(_sql, "table_id"))
                .ok_or_else(|| RockLakeError::MissingParam {
                    name: "table_id".to_string(),
                })?;
            let reader = resolve_reader(store, session, u64::MAX).await?;
            let files = reader
                .list_data_files(table_id)
                .await
                .map_err(RockLakeError::from)?;
            let deletes = reader
                .list_delete_files(table_id)
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_extended_data_file_response(files, deletes)])
        }
        StatementKind::SelectDataFiles => {
            let stable_1_1 = stable_ducklake_v11(store).await?;
            let table_id = resolve_comparison_u64(_sql, "table_id", params);
            let snap_id =
                resolve_comparison_u64(_sql, "begin_snapshot", params).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;

            // DuckLake's JOIN-based file scanner expects data_path/del_path
            // aliases, not the raw ducklake_data_file metadata table schema.
            // Preserve the simple metadata scans used by older clients.
            let lower = _sql.to_ascii_lowercase();
            if lower.contains("ducklake_data_file")
                && lower.contains("ducklake_delete_file")
                && lower.contains("data_unsupported_format")
                && lower.contains("del_path")
            {
                let table_id = table_id
                    .or_else(|| literal_assignment_u64(_sql, "table_id"))
                    .ok_or_else(|| RockLakeError::MissingParam {
                        name: "table_id".to_string(),
                    })?;
                let files = reader
                    .list_data_files(table_id)
                    .await
                    .map_err(RockLakeError::from)?;
                let deletes = reader
                    .list_delete_files(table_id)
                    .await
                    .map_err(RockLakeError::from)?;
                return Ok(vec![make_scan_data_file_response(files, deletes, _sql)]);
            }

            if let Some(table_id) = table_id {
                return Ok(vec![
                    make_data_files_stream_response(reader, table_id, None, _sql, stable_1_1)
                        .await?,
                ]);
            } else {
                let schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
                let mut table_ids = Vec::new();
                for schema in schemas {
                    for table in reader
                        .list_tables(schema.schema_id)
                        .await
                        .map_err(RockLakeError::from)?
                    {
                        table_ids.push(table.table_id);
                    }
                }
                let snapshot_id = reader.snapshot_id().as_u64();
                let streams = futures::stream::iter(table_ids).then(move |table_id| {
                    let reader = reader.clone();
                    async move { reader.stream_data_files(table_id).await }
                });
                let files = streams.try_flatten().boxed();
                Ok(vec![make_data_files_stream_response_from_rows(
                    snapshot_id,
                    files,
                    None,
                    _sql,
                    stable_1_1,
                )])
            }
        }
        StatementKind::SelectFileColumnStats => {
            let stable_1_1 = stable_ducklake_v11(store).await?;
            let table_id = params
                .get_u64(0)
                .ok()
                .or_else(|| literal_assignment_u64(_sql, "table_id"));
            let column_id = params
                .get_u64(1)
                .ok()
                .or_else(|| literal_assignment_u64(_sql, "column_id"));
            let (Some(table_id), Some(column_id)) = (table_id, column_id) else {
                return Ok(vec![make_metadata_table_empty_response(
                    "ducklake_file_column_stats",
                    stable_1_1,
                )]);
            };
            let snap_id = params.get_u64(2).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let predicate = params.get(3).unwrap_or("");
            if predicate.is_empty() {
                let rows = reader
                    .stream_file_column_stats(table_id, column_id)
                    .await
                    .map_err(RockLakeError::from)?;
                return Ok(vec![make_file_column_stats_stream_response(
                    reader.snapshot_id().as_u64(),
                    _sql,
                    stable_1_1,
                    rows,
                )]);
            }
            // v0.26: look up the actual column type for type-aware pruning.
            let col_type = reader
                .get_column_type(table_id, column_id)
                .await
                .map_err(RockLakeError::from)?
                .as_deref()
                .map(rocklake_core::types::DuckLakeType::parse)
                .unwrap_or(rocklake_core::types::DuckLakeType::Varchar);
            let file_ids = reader
                .prune_files(table_id, column_id, predicate, &col_type)
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_file_ids_response(file_ids)])
        }
        StatementKind::SelectTableStats => {
            // DuckDB postgres_query() sends literal WHERE table_id predicates,
            // not bind parameters. Honor those predicates before falling back
            // to the legacy positional client parameters.
            let table_id =
                resolve_comparison_u64(_sql, "table_id", params).or_else(|| params.get_u64(0).ok());
            let snap_id = params.get_u64(1).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            if _sql
                .to_ascii_lowercase()
                .contains("ducklake_table_column_stats")
            {
                let stable_1_1 = stable_ducklake_v11(store).await?;
                let table_id = table_id.or_else(|| literal_assignment_u64(_sql, "table_id"));
                let stats_rows = if let Some(table_id) = table_id {
                    reader
                        .get_table_stats(table_id)
                        .await
                        .map_err(RockLakeError::from)?
                        .into_iter()
                        .collect()
                } else {
                    reader
                        .list_all_table_stats()
                        .await
                        .map_err(RockLakeError::from)?
                };
                let mut column_stats = reader
                    .list_all_table_column_stats()
                    .await
                    .map_err(RockLakeError::from)?;
                if let Some(table_id) = table_id {
                    column_stats.retain(|row| row.table_id == table_id);
                }
                return Ok(vec![make_global_table_stats_response(
                    stats_rows,
                    column_stats,
                    stable_1_1,
                )]);
            }
            if let Some(table_id) = table_id {
                let stats = reader
                    .get_table_stats(table_id)
                    .await
                    .map_err(RockLakeError::from)?;
                Ok(vec![make_table_stats_rows_response_for_sql(
                    _sql,
                    stats.into_iter().collect(),
                )])
            } else {
                let rows = reader
                    .list_all_table_stats()
                    .await
                    .map_err(RockLakeError::from)?;
                Ok(vec![make_table_stats_rows_response_for_sql(_sql, rows)])
            }
        }
        StatementKind::SelectTableColumnStats => {
            let stable_1_1 = stable_ducklake_v11(store).await?;
            let reader = { store.lock().await.read_latest() };
            let rows = reader
                .list_all_table_column_stats()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_table_column_stats_response(rows, stable_1_1)])
        }
        StatementKind::SelectDeleteFiles => {
            let stable_1_1 = stable_ducklake_v11(store).await?;
            // Do not return other tables' delete files for a literal filter.
            // Incorrect cross-table deletion counts can produce a negative net
            // row count, which DuckDB 2.0 rejects as an unsigned value.
            let table_id =
                resolve_comparison_u64(_sql, "table_id", params).or_else(|| params.get_u64(0).ok());
            let snap_id = params.get_u64(1).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            if let Some(table_id) = table_id {
                let files = reader
                    .stream_delete_files(table_id)
                    .await
                    .map_err(RockLakeError::from)?;
                return Ok(vec![make_delete_files_stream_response(
                    reader.snapshot_id().as_u64(),
                    files,
                    _sql,
                    stable_1_1,
                )]);
            }
            let schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
            let mut table_ids = Vec::new();
            for schema in schemas {
                for table in reader
                    .list_tables(schema.schema_id)
                    .await
                    .map_err(RockLakeError::from)?
                {
                    table_ids.push(table.table_id);
                }
            }
            let snapshot_id = reader.snapshot_id().as_u64();
            let streams = futures::stream::iter(table_ids).then(move |table_id| {
                let reader = reader.clone();
                async move { reader.stream_delete_files(table_id).await }
            });
            let files = streams.try_flatten().boxed();
            Ok(vec![make_delete_files_stream_response(
                snapshot_id,
                files,
                _sql,
                stable_1_1,
            )])
        }
        StatementKind::SelectSnapshotHistory => {
            let reader = { store.lock().await.read_latest() };
            let rows = reader
                .list_all_snapshots()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_snapshot_history_response(_sql, rows)])
        }
        StatementKind::SelectSnapshot => {
            let snap_id = params.get_u64(0).ok();
            let reader = {
                let s = store.lock().await;
                if let Some(id) = snap_id {
                    s.read_at(rocklake_core::mvcc::SnapshotId::new(id))
                        .map_err(RockLakeError::from)?
                } else {
                    // No snapshot ID provided (e.g. SELECT * FROM ducklake_snapshot):
                    // return the latest committed snapshot.
                    s.read_latest()
                }
            };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            if let Some(snap) = snap {
                Ok(vec![make_snapshot_row_response(snap)])
            } else {
                Ok(vec![make_empty_response()])
            }
        }
        StatementKind::SelectSnapshotChanges => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let rows = reader
                .list_all_snapshot_changes()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_snapshot_changes_response(rows)])
        }
        StatementKind::SelectMetadata => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let key = resolve_comparison_string(_sql, "key", params);
            let scope = resolve_comparison_string(_sql, "scope", params);
            let scope_id = resolve_comparison_u64(_sql, "scope_id", params);
            let mut rows = reader
                .list_all_metadata()
                .await
                .map_err(RockLakeError::from)?;
            if let Some(key) = key {
                rows.retain(|row| row.key == key);
            }
            if let Some(scope) = scope {
                rows.retain(|row| row.scope.as_deref().unwrap_or("global") == scope);
            }
            if let Some(scope_id) = scope_id {
                rows.retain(|row| row.scope_id.unwrap_or(0) == scope_id);
            }
            session.metadata_ctid_rows = if _sql.to_ascii_lowercase().contains("ctid") {
                rows.clone()
            } else {
                Vec::new()
            };
            Ok(vec![make_metadata_response(_sql, rows)])
        }
        StatementKind::SelectViews => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let rows = reader.list_all_views().await.map_err(RockLakeError::from)?;
            Ok(vec![make_views_response(rows)])
        }
        StatementKind::SelectMacros => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let rows = reader
                .list_all_macros()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_macros_response(rows)])
        }
        StatementKind::SelectMacroImpls => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let macros = reader
                .list_all_macros()
                .await
                .map_err(RockLakeError::from)?;
            let mut rows = Vec::new();
            for macro_row in macros {
                rows.extend(
                    reader
                        .list_macro_impls(macro_row.macro_id)
                        .await
                        .map_err(RockLakeError::from)?,
                );
            }
            Ok(vec![make_macro_impls_response(rows)])
        }
        StatementKind::SelectMacroParameters => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let macros = reader
                .list_all_macros()
                .await
                .map_err(RockLakeError::from)?;
            let mut rows = Vec::new();
            for macro_row in macros {
                for impl_row in reader
                    .list_macro_impls(macro_row.macro_id)
                    .await
                    .map_err(RockLakeError::from)?
                {
                    rows.extend(
                        reader
                            .list_macro_parameters(impl_row.macro_id, impl_row.impl_id)
                            .await
                            .map_err(RockLakeError::from)?,
                    );
                }
            }
            Ok(vec![make_macro_parameters_response(rows)])
        }

        // ─── v0.27: ducklake_tag / ducklake_column_tag / ducklake_sort_info ─
        StatementKind::SelectTags => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let rows = reader.list_all_tags().await.map_err(RockLakeError::from)?;
            Ok(vec![make_tags_response(rows)])
        }
        StatementKind::SelectColumnTags => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let rows = reader
                .list_all_column_tags()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_column_tags_response(rows)])
        }
        StatementKind::SelectSortInfo => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let rows = reader
                .list_all_sort_info()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_sort_info_response(rows)])
        }
        StatementKind::SelectSchemaVersion => {
            let catalog_version = { store.lock().await.schema_version() };
            Ok(vec![make_schema_version_response(catalog_version)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_schema_versions" =>
        {
            let table_id = resolve_comparison_u64(_sql, "table_id", params);
            let schema_version = resolve_comparison_u64(_sql, "schema_version", params);
            let begin_snapshot = resolve_comparison_u64(_sql, "begin_snapshot", params);
            let reader = { store.lock().await.read_latest() };
            let mut rows = reader
                .list_all_schema_versions()
                .await
                .map_err(RockLakeError::from)?;
            if let Some(table_id) = table_id {
                rows.retain(|row| row.table_id == table_id);
            }
            if let Some(schema_version) = schema_version {
                rows.retain(|row| row.schema_version == schema_version);
            }
            if let Some(begin_snapshot) = begin_snapshot {
                rows.retain(|row| row.begin_snapshot <= begin_snapshot);
            }
            Ok(vec![make_schema_versions_response(_sql, rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_view_column_tag" =>
        {
            let reader = { store.lock().await.read_latest() };
            let version = reader
                .get_metadata(rocklake_core::keys::MetadataScope::Global, 0, "version")
                .await
                .map_err(RockLakeError::from)?
                .map(|row| row.value);
            if !matches!(version.as_deref(), Some("1.1" | "1.1-dev1")) {
                return Err(RockLakeError::Unsupported(
                    "ducklake_view_column_tag requires DuckLake format version 1.1".into(),
                ));
            }
            let rows = reader
                .list_all_view_column_tags()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_view_column_tags_response(rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_file_variant_stats" =>
        {
            let _reader = { store.lock().await.read_latest() };
            // Return empty for now; variant stats are typically queried with WHERE clauses by table_id/column_id
            let rows = Vec::new();
            Ok(vec![make_file_variant_stats_response(rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_column_mapping" =>
        {
            let reader = { store.lock().await.read_latest() };
            let rows = reader
                .list_column_mappings()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_column_mapping_response(rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_name_mapping" =>
        {
            let reader = { store.lock().await.read_latest() };
            let rows = reader
                .list_name_mappings()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_name_mapping_response(rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_partition_info" =>
        {
            let table_id = resolve_comparison_u64(_sql, "table_id", params);
            let snapshot_id =
                resolve_comparison_u64(_sql, "begin_snapshot", params).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snapshot_id).await?;
            let rows = if let Some(table_id) = table_id {
                reader
                    .list_partition_info(table_id)
                    .await
                    .map_err(RockLakeError::from)?
            } else {
                let schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
                let mut rows = Vec::new();
                for schema in schemas {
                    for table in reader
                        .list_tables(schema.schema_id)
                        .await
                        .map_err(RockLakeError::from)?
                    {
                        rows.extend(
                            reader
                                .list_partition_info(table.table_id)
                                .await
                                .map_err(RockLakeError::from)?,
                        );
                    }
                }
                rows
            };
            Ok(vec![make_partition_info_response(_sql, rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_partition_column" =>
        {
            let partition_id = params.get_u64(0).ok();
            let reader = { store.lock().await.read_latest() };
            let rows = if let Some(partition_id) = partition_id {
                reader
                    .list_partition_columns(partition_id)
                    .await
                    .map_err(RockLakeError::from)?
            } else {
                reader
                    .list_all_partition_columns()
                    .await
                    .map_err(RockLakeError::from)?
            };
            Ok(vec![make_partition_columns_response(_sql, rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_file_partition_value" =>
        {
            let table_id = resolve_comparison_u64(_sql, "table_id", params);
            let reader = { store.lock().await.read_latest() };
            let rows = reader
                .list_file_partition_values(table_id)
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_file_partition_values_response(_sql, rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_sort_expression" =>
        {
            let table_id = params.get_u64(0).ok();
            let reader = { store.lock().await.read_latest() };
            let rows = if let Some(table_id) = table_id {
                reader
                    .list_sort_expressions(table_id)
                    .await
                    .map_err(RockLakeError::from)?
            } else {
                reader
                    .list_all_sort_expressions()
                    .await
                    .map_err(RockLakeError::from)?
            };
            Ok(vec![make_sort_expressions_response(rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name }
            if table_name == "ducklake_files_scheduled_for_deletion" =>
        {
            let reader = { store.lock().await.read_latest() };
            let mut rows = reader
                .list_files_scheduled_for_deletion()
                .await
                .map_err(RockLakeError::from)?;
            let lower = _sql.to_ascii_lowercase();
            if let Some(pos) = lower
                .find("schedule_start\"")
                .or_else(|| lower.find("schedule_start"))
            {
                if let Some(after) = _sql[pos..].split_once('<').map(|(_, s)| s) {
                    let timestamp = after.split('\'').nth(1).ok_or_else(|| {
                        RockLakeError::ValueDecode("missing cleanup cutoff".into())
                    })?;
                    let cutoff = chrono::DateTime::parse_from_rfc3339(timestamp)
                        .or_else(|_| {
                            chrono::DateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S%.f%#z")
                        })
                        .map_err(|_| RockLakeError::ValueDecode("invalid cleanup cutoff".into()))?
                        .timestamp_micros();
                    rows.retain(|row| {
                        i128::from(row.schedule_start) * 1_000_000 < i128::from(cutoff)
                    });
                }
            }
            let has_ctid = lower.contains("ctid");
            if has_ctid {
                if let Some((_, predicates)) = lower.split_once("where") {
                    for predicate in predicates.split("and") {
                        let predicate = predicate
                            .trim()
                            .trim_matches(|c| c == '(' || c == ')')
                            .trim();
                        let predicate = predicate
                            .strip_prefix("\"data_file_id\"")
                            .or_else(|| predicate.strip_prefix("data_file_id"))
                            .ok_or_else(|| {
                                RockLakeError::Unsupported(
                                    "unsupported cleanup file predicate".into(),
                                )
                            })?
                            .trim();
                        if let Some(value) =
                            predicate.strip_prefix(">=").and_then(parse_leading_u64)
                        {
                            rows.retain(|row| row.data_file_id >= value);
                        } else if let Some(value) =
                            predicate.strip_prefix("<=").and_then(parse_leading_u64)
                        {
                            rows.retain(|row| row.data_file_id <= value);
                        } else {
                            return Err(RockLakeError::Unsupported(
                                "unsupported cleanup file predicate".into(),
                            ));
                        }
                    }
                }
            }
            if has_ctid && rows.iter().any(|row| row.data_file_id > u32::MAX as u64) {
                return Err(RockLakeError::Unsupported(
                    "cleanup file ID exceeds ctid range".into(),
                ));
            }
            Ok(vec![make_files_scheduled_for_deletion_response(
                rows, has_ctid,
            )])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name } => {
            Ok(vec![make_metadata_table_empty_response(
                table_name,
                stable_ducklake_v11(store).await?,
            )])
        }

        StatementKind::SelectInlinedData => {
            let table_id = resolve_comparison_u64(_sql, "table_id", params);
            let schema_version = resolve_comparison_u64(_sql, "schema_version", params);
            let reader = { store.lock().await.read_latest() };
            let mut rows = reader
                .list_inlined_data_tables(table_id)
                .await
                .map_err(RockLakeError::from)?;
            if let Some(schema_version) = schema_version {
                rows.retain(|row| row.schema_version == schema_version);
            }
            Ok(vec![make_inlined_data_tables_response(_sql, rows)])
        }
        StatementKind::SelectInlinedRows => {
            session.inlined_delete_ctid_rows.clear();
            let Some(table_name) = inlined_table_name_from_sql(_sql) else {
                return Ok(vec![make_empty_response()]);
            };
            if let Some(table_id) = parse_inlined_delete_table_id(&table_name) {
                let snap_id =
                    resolve_comparison_u64(_sql, "begin_snapshot", params).unwrap_or(u64::MAX);
                let reader = resolve_reader(store, session, snap_id).await?;
                let rows = reader
                    .list_inlined_deletes(table_id)
                    .await
                    .map_err(RockLakeError::from)?;
                if _sql.to_ascii_lowercase().contains("ctid") {
                    session.inlined_delete_ctid_rows = rows.clone();
                }
                return Ok(vec![make_inlined_deletes_response(_sql, rows)]);
            }
            let Some((table_id, schema_version)) = parse_inlined_table_ids(&table_name) else {
                return Ok(vec![make_empty_response()]);
            };
            let stable_1_1 = stable_ducklake_v11(store).await?;
            let begin_snapshot_column = if stable_1_1 {
                "_ducklake_begin_snapshot"
            } else {
                "begin_snapshot"
            };
            let snap_id =
                resolve_comparison_u64(_sql, begin_snapshot_column, params).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let Some((_, columns)) = reader
                .describe_table(table_id)
                .await
                .map_err(RockLakeError::from)?
            else {
                return Ok(vec![make_empty_response()]);
            };
            let selection = inlined_row_selection(_sql, stable_1_1);
            let raw_inserts = match selection {
                InlinedRowSelection::Visible => reader.list_inlined_inserts(table_id).await,
                _ => reader.list_inlined_insert_versions(table_id).await,
            }
            .map_err(RockLakeError::from)?;
            let rows = raw_inserts
                .into_iter()
                .filter(|row| {
                    row.schema_version == schema_version
                        && match selection {
                            InlinedRowSelection::Open => row.end_snapshot.is_none(),
                            InlinedRowSelection::Ended => row.end_snapshot.is_some(),
                            _ => true,
                        }
                })
                .collect::<Vec<_>>();
            Ok(vec![make_inlined_rows_response(
                _sql, columns, rows, stable_1_1,
            )])
        }

        // ─── pg-tide-relay extensions ──────────────────────────────────
        StatementKind::SelectMaxSnapshotAfter => {
            let after_id = params.get_u64(0).unwrap_or(0);
            let reader = { store.lock().await.read_latest() };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            let id = snap.map(|s| s.snapshot_id).unwrap_or(0);
            if id > after_id {
                Ok(vec![make_single_int_response("max", Some(id as i64))])
            } else {
                Ok(vec![make_single_int_response("max", None)])
            }
        }
        StatementKind::SelectFirstSnapshot => {
            let snap = {
                let s = store.lock().await;
                match s.read_at(rocklake_core::mvcc::SnapshotId::new(1)) {
                    Ok(reader) => reader.get_snapshot().await.map_err(RockLakeError::from)?,
                    Err(rocklake_catalog::error::CatalogError::SnapshotNotFound { .. }) => None,
                    Err(e) => return Err(RockLakeError::from(e)),
                }
            };
            Ok(vec![make_latest_snapshot_info_response(snap, false, None)])
        }
        StatementKind::SelectDataFilesWithLimit => {
            let table_id = require_param_u64(params, 0, "table_id")?;
            let limit = params.get_u64(1).unwrap_or(u64::MAX);
            let snap_id = params.get_u64(2).unwrap_or(u64::MAX);
            let reader = resolve_reader(store, session, snap_id).await?;
            let limit = usize::try_from(limit).unwrap_or(usize::MAX);
            Ok(vec![
                make_data_files_stream_response(
                    reader,
                    table_id,
                    Some(limit),
                    _sql,
                    stable_ducklake_v11(store).await?,
                )
                .await?,
            ])
        }
        StatementKind::SelectGenRandomUuid => {
            let uuid_val = uuid::Uuid::new_v4().to_string();
            Ok(vec![make_single_text_response(
                "gen_random_uuid",
                Some(&uuid_val),
            )])
        }

        // ─── Write Operations (buffered in transaction) ────────────────
        StatementKind::InsertSnapshot => {
            let op = BufferedOp::InsertSnapshot {
                author: params.get_optional_string(0),
                message: params.get_optional_string(1),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertSnapshotChanges => {
            let literals = literal_insert_values(_sql);
            let op = BufferedOp::InsertSnapshotChanges {
                change_type: if _sql.contains('$') {
                    required_param_string(params, 0, "change_type")?
                } else {
                    literal_string(&literals, 1)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "change_type".into(),
                        })?
                },
                change_info: params
                    .get_optional_string(1)
                    .or_else(|| literal_string(&literals, 4)),
                schema_id: params.get_u64(2).ok(),
                table_id: params.get_u64(3).ok(),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertSchema => {
            let stable_1_1 = {
                let catalog = store.lock().await;
                crate::schema_registry::is_ducklake_v11(&catalog)
                    .await
                    .map_err(RockLakeError::from)?
            };
            let ops = if let Some(rows) = parse_insert_rows_map(_sql, params) {
                rows.into_iter()
                    .map(|row| {
                        let schema_name = row
                            .get("schema_name")
                            .and_then(|value| value.clone())
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| RockLakeError::MissingParam {
                                name: "schema_name".into(),
                            })?;
                        let parent_schema_id = match row.get("parent_schema_id") {
                            Some(Some(value)) => Some(value.parse::<u64>().map_err(|_| {
                                RockLakeError::MissingParam {
                                    name: "parent_schema_id".into(),
                                }
                            })?),
                            _ => None,
                        };
                        if parent_schema_id.is_some() && !stable_1_1 {
                            return Err(RockLakeError::Unsupported(
                                "nested schemas require DuckLake 1.1".into(),
                            ));
                        }
                        let path_is_relative = match row.get("path_is_relative") {
                            Some(Some(_)) => {
                                Some(row_map_bool(&row, "path_is_relative").ok_or_else(|| {
                                    RockLakeError::MissingParam {
                                        name: "path_is_relative".into(),
                                    }
                                })?)
                            }
                            Some(None) => None,
                            None => Some(true),
                        };
                        Ok(BufferedOp::InsertSchema {
                            schema_name,
                            schema_uuid: row_map_string(&row, "schema_uuid"),
                            path: row_map_string(&row, "path"),
                            path_is_relative,
                            parent_schema_id,
                        })
                    })
                    .collect::<Result<Vec<_>, RockLakeError>>()?
            } else {
                let literals = literal_insert_values(_sql);
                let schema_name = params
                    .get_string(0)
                    .ok()
                    .or_else(|| literal_string(&literals, 4))
                    .unwrap_or_default();
                if schema_name.is_empty() {
                    return Err(RockLakeError::MissingParam {
                        name: "schema_name".into(),
                    });
                }
                vec![BufferedOp::InsertSchema {
                    schema_name,
                    schema_uuid: None,
                    path: None,
                    path_is_relative: None,
                    parent_schema_id: None,
                }]
            };
            let row_count = submit_ops(ops, session, store, notify_manager).await?;
            Ok(vec![Response::Execution(Tag::new(&format!(
                "INSERT 0 {row_count}"
            )))])
        }
        StatementKind::InsertTable => {
            let mut table_id = None;
            let mut schema_id = 1;
            let mut table_name = String::new();
            let mut data_path = None;

            if let Some(rows) = parse_insert_rows_map(_sql, params) {
                if let Some(row) = rows.first() {
                    table_id = row
                        .get("table_id")
                        .and_then(|v| v.as_ref().and_then(|s| s.parse::<u64>().ok()));
                    schema_id = row
                        .get("schema_id")
                        .and_then(|v| v.as_ref().and_then(|s| s.parse::<u64>().ok()))
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "schema_id".into(),
                        })?;
                    table_name = row
                        .get("table_name")
                        .and_then(|v| v.clone())
                        .unwrap_or_default();
                    data_path = row
                        .get("path")
                        .and_then(|v| v.clone())
                        .or_else(|| row.get("data_path").and_then(|v| v.clone()));
                }
            } else {
                let literals = literal_insert_values(_sql);
                schema_id = if _sql.contains('$') {
                    required_param_u64(params, 0, "schema_id")?
                } else {
                    literal_u64(&literals, 4).ok_or_else(|| RockLakeError::MissingParam {
                        name: "schema_id".into(),
                    })?
                };
                table_name = params
                    .get_string(1)
                    .ok()
                    .or_else(|| literal_string(&literals, 5))
                    .unwrap_or_default();
                data_path = params
                    .get_optional_string(2)
                    .or_else(|| literal_string(&literals, 6));
            }

            if table_name.is_empty() {
                return Err(RockLakeError::MissingParam {
                    name: "table_name".into(),
                });
            }
            let op = BufferedOp::InsertTable {
                table_id,
                schema_id,
                table_name,
                data_path,
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertColumn => {
            let mut ops = Vec::new();
            if let Some(rows) = parse_insert_rows_map(_sql, params) {
                for row in rows {
                    let column_id = row
                        .get("column_id")
                        .and_then(|v| v.as_ref().and_then(|s| s.parse::<u64>().ok()));
                    let table_id = row_map_u64(&row, "table_id").ok_or_else(|| {
                        RockLakeError::MissingParam {
                            name: "table_id".into(),
                        }
                    })?;
                    let column_name = row
                        .get("column_name")
                        .and_then(|v| v.clone())
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "column_name".into(),
                        })?;
                    let data_type = row
                        .get("column_type")
                        .and_then(|v| v.clone())
                        .or_else(|| row.get("data_type").and_then(|v| v.clone()))
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "data_type".into(),
                        })?;
                    let column_index = row
                        .get("column_order")
                        .and_then(|v| v.as_ref().and_then(|s| s.parse::<u64>().ok()))
                        .or_else(|| {
                            row.get("column_index")
                                .and_then(|v| v.as_ref().and_then(|s| s.parse::<u64>().ok()))
                        })
                        .unwrap_or(0);
                    let is_nullable = row
                        .get("nulls_allowed")
                        .and_then(|v| v.as_ref().and_then(|s| s.parse::<bool>().ok()))
                        .or_else(|| {
                            row.get("is_nullable")
                                .and_then(|v| v.as_ref().and_then(|s| s.parse::<bool>().ok()))
                        })
                        .unwrap_or(true);
                    let default_value = row.get("default_value").and_then(|v| v.clone());
                    let initial_default = row.get("initial_default").and_then(|v| v.clone());
                    let default_value_type = row.get("default_value_type").and_then(|v| v.clone());
                    let default_value_dialect =
                        row.get("default_value_dialect").and_then(|v| v.clone());
                    let parent_column = row
                        .get("parent_column")
                        .and_then(|v| v.as_ref().and_then(|s| s.parse::<u64>().ok()));

                    ops.push(BufferedOp::InsertColumn {
                        column_id,
                        table_id,
                        column_name,
                        data_type,
                        column_index,
                        is_nullable,
                        default_value,
                        initial_default,
                        default_value_type,
                        default_value_dialect,
                        parent_column,
                    });
                }
            } else {
                let literal_rows = literal_insert_rows(_sql);
                let parameterized = params.get_u64(0).is_ok();
                if parameterized || literal_rows.is_empty() {
                    let literals = literal_rows.into_iter().next().unwrap_or_default();
                    ops.push(BufferedOp::InsertColumn {
                        column_id: None,
                        table_id: if parameterized {
                            required_param_u64(params, 0, "table_id")?
                        } else {
                            literal_u64(&literals, 3).ok_or_else(|| {
                                RockLakeError::MissingParam {
                                    name: "table_id".into(),
                                }
                            })?
                        },
                        column_name: if parameterized {
                            required_param_string(params, 1, "column_name")?
                        } else {
                            literal_string(&literals, 5)
                                .filter(|value| !value.is_empty())
                                .ok_or_else(|| RockLakeError::MissingParam {
                                    name: "column_name".into(),
                                })?
                        },
                        data_type: if parameterized {
                            required_param_string(params, 2, "data_type")?
                        } else {
                            literal_string(&literals, 6)
                                .filter(|value| !value.is_empty())
                                .ok_or_else(|| RockLakeError::MissingParam {
                                    name: "data_type".into(),
                                })?
                        },
                        column_index: if parameterized {
                            required_param_u64(params, 3, "column_index")?
                        } else {
                            literal_u64(&literals, 4).ok_or_else(|| {
                                RockLakeError::MissingParam {
                                    name: "column_index".into(),
                                }
                            })?
                        },
                        is_nullable: if parameterized {
                            required_param_bool(params, 4, "is_nullable")?
                        } else {
                            literal_bool(&literals, 9).ok_or_else(|| {
                                RockLakeError::MissingParam {
                                    name: "is_nullable".into(),
                                }
                            })?
                        },
                        default_value: params
                            .get_optional_string(5)
                            .or_else(|| literal_string(&literals, 8)),
                        initial_default: params
                            .get_optional_string(6)
                            .or_else(|| literal_string(&literals, 7)),
                        default_value_type: params
                            .get_optional_string(7)
                            .or_else(|| literal_string(&literals, 11)),
                        default_value_dialect: params
                            .get_optional_string(8)
                            .or_else(|| literal_string(&literals, 12)),
                        parent_column: params
                            .get_u64(9)
                            .ok()
                            .or_else(|| literal_u64(&literals, 10)),
                    });
                } else {
                    for literals in literal_rows {
                        ops.push(BufferedOp::InsertColumn {
                            column_id: None,
                            table_id: literal_u64(&literals, 3).unwrap_or(0),
                            column_name: literal_string(&literals, 5).unwrap_or_default(),
                            data_type: literal_string(&literals, 6).unwrap_or_default(),
                            column_index: literal_u64(&literals, 4).unwrap_or(0),
                            is_nullable: literal_bool(&literals, 9).unwrap_or(true),
                            default_value: literal_string(&literals, 8),
                            initial_default: literal_string(&literals, 7),
                            default_value_type: literal_string(&literals, 11),
                            default_value_dialect: literal_string(&literals, 12),
                            parent_column: literal_u64(&literals, 10),
                        });
                    }
                }
            }
            if ops.iter().any(|op| {
                matches!(op, BufferedOp::InsertColumn { table_id: 0, .. })
                    || matches!(
                        op,
                        BufferedOp::InsertColumn {
                            column_name,
                            data_type,
                            ..
                        } if column_name.is_empty() || data_type.is_empty()
                    )
            }) {
                return Err(RockLakeError::MissingParam {
                    name: "column metadata".into(),
                });
            }
            let row_count = ops.len();
            if session.in_transaction {
                for op in ops {
                    session.pending_txn.push(op)?;
                }
            } else {
                execute_commit(ops, store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "INSERT 0 {row_count}"
            )))])
        }
        StatementKind::InsertDataFile => {
            if let Some(rows) = parse_insert_rows_map(_sql, params)
                .filter(|rows| rows.iter().any(|row| !row.is_empty()))
            {
                let ops = rows
                    .into_iter()
                    .map(|row| {
                        Ok(BufferedOp::InsertDataFile {
                            data_file_id: row_map_u64(&row, "data_file_id"),
                            table_id: required_row_u64_value(&row, &["table_id"])?,
                            path: required_row_string_value(&row, &["path"])?,
                            file_format: required_row_string_value(
                                &row,
                                &["file_format", "format"],
                            )?,
                            row_count: required_row_u64_value(
                                &row,
                                &["record_count", "row_count"],
                            )?,
                            file_size_bytes: required_row_u64_value(&row, &["file_size_bytes"])?,
                            row_id_start: row_map_u64(&row, "row_id_start"),
                            row_id_start_supplied: row.contains_key("row_id_start"),
                            begin_snapshot: row_map_u64(&row, "begin_snapshot"),
                            end_snapshot: row_map_u64(&row, "end_snapshot"),
                            file_order: row_map_u64(&row, "file_order"),
                            path_is_relative: row_map_bool(&row, "path_is_relative"),
                            footer_size: optional_row_i64_value(&row, "footer_size")?,
                            encryption_key: row_map_string(&row, "encryption_key"),
                            partition_id: row_map_u64(&row, "partition_id"),
                            mapping_id: row_map_u64(&row, "mapping_id"),
                            partial_max: row_map_string(&row, "partial_max"),
                            row_group_count: row_map_u64(&row, "row_group_count"),
                        })
                    })
                    .collect::<Result<Vec<_>, RockLakeError>>()?;
                let row_count = submit_ops(ops, session, store, notify_manager).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            let literals = literal_insert_values(_sql);
            let parameterized = _sql.contains('$');
            let op = BufferedOp::InsertDataFile {
                data_file_id: None,
                table_id: if parameterized {
                    required_param_u64(params, 0, "table_id")?
                } else {
                    literal_u64(&literals, 1).ok_or_else(|| RockLakeError::MissingParam {
                        name: "table_id".into(),
                    })?
                },
                path: if parameterized {
                    required_param_string(params, 1, "path")?
                } else {
                    literal_string(&literals, 5)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "path".into(),
                        })?
                },
                file_format: if parameterized {
                    required_param_string(params, 2, "file_format")?
                } else {
                    literal_string(&literals, 7).unwrap_or_else(|| "parquet".to_string())
                },
                row_count: if parameterized {
                    required_param_u64(params, 3, "row_count")?
                } else {
                    literal_u64(&literals, 8).ok_or_else(|| RockLakeError::MissingParam {
                        name: "row_count".into(),
                    })?
                },
                file_size_bytes: if parameterized {
                    required_param_u64(params, 4, "file_size_bytes")?
                } else {
                    literal_u64(&literals, 9).ok_or_else(|| RockLakeError::MissingParam {
                        name: "file_size_bytes".into(),
                    })?
                },
                row_id_start: if parameterized {
                    None
                } else {
                    literal_u64(&literals, 11)
                },
                row_id_start_supplied: !parameterized && literals.len() > 11,
                begin_snapshot: None,
                end_snapshot: None,
                file_order: None,
                path_is_relative: None,
                footer_size: literal_string(&literals, 10)
                    .and_then(|value| value.parse::<i64>().ok()),
                encryption_key: None,
                partition_id: None,
                mapping_id: None,
                partial_max: None,
                row_group_count: None,
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertDeleteFile => {
            if let Some(rows) = parse_insert_rows_map(_sql, params)
                .filter(|rows| rows.iter().any(|row| !row.is_empty()))
            {
                let ops = rows
                    .into_iter()
                    .map(|row| {
                        Ok(BufferedOp::InsertDeleteFile {
                            data_file_id: row_map_u64(&row, "data_file_id").unwrap_or(0),
                            begin_snapshot: row_map_u64(&row, "begin_snapshot"),
                            path: required_row_string_value(&row, &["path"])?,
                            delete_count: required_row_u64_value(
                                &row,
                                &["delete_count", "row_count"],
                            )?,
                            file_size_bytes: required_row_u64_value(&row, &["file_size_bytes"])?,
                            table_id: row_map_u64(&row, "table_id"),
                            format: row_map_string(&row, "format"),
                            path_is_relative: row_map_bool(&row, "path_is_relative"),
                            footer_size: optional_row_i64_value(&row, "footer_size")?,
                            partial_max: row_map_string(&row, "partial_max"),
                            encryption_key: row_map_string(&row, "encryption_key"),
                            row_group_count: row_map_u64(&row, "row_group_count"),
                        })
                    })
                    .collect::<Result<Vec<_>, RockLakeError>>()?;
                let row_count = submit_ops(ops, session, store, notify_manager).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            let parameterized = _sql.contains('$');
            let literals = literal_insert_values(_sql);
            let op = BufferedOp::InsertDeleteFile {
                data_file_id: if parameterized {
                    required_param_u64(params, 0, "data_file_id")?
                } else {
                    literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                        name: "data_file_id".into(),
                    })?
                },
                begin_snapshot: None,
                path: if parameterized {
                    required_param_string(params, 1, "path")?
                } else {
                    literal_string(&literals, 1)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "path".into(),
                        })?
                },
                delete_count: if parameterized {
                    required_param_u64(params, 2, "delete_count")?
                } else {
                    literal_u64(&literals, 2).ok_or_else(|| RockLakeError::MissingParam {
                        name: "delete_count".into(),
                    })?
                },
                file_size_bytes: if parameterized {
                    required_param_u64(params, 3, "file_size_bytes")?
                } else {
                    literal_u64(&literals, 3).ok_or_else(|| RockLakeError::MissingParam {
                        name: "file_size_bytes".into(),
                    })?
                },
                table_id: None,
                format: None,
                path_is_relative: None,
                footer_size: None,
                partial_max: None,
                encryption_key: None,
                row_group_count: None,
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertTableStats => {
            let literals = literal_insert_values(_sql);
            let parameterized = _sql.contains('$');
            let op = BufferedOp::InsertTableStats {
                table_id: if parameterized {
                    required_param_u64(params, 0, "table_id")?
                } else {
                    literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                        name: "table_id".into(),
                    })?
                },
                record_count: if parameterized {
                    required_param_u64(params, 1, "record_count")?
                } else {
                    literal_u64(&literals, 1).ok_or_else(|| RockLakeError::MissingParam {
                        name: "record_count".into(),
                    })?
                },
                // DuckLake v1.0 position 2 is next_row_id, not file_count.
                next_row_id: if parameterized {
                    required_param_u64(params, 2, "next_row_id")?
                } else {
                    literal_u64(&literals, 2).ok_or_else(|| RockLakeError::MissingParam {
                        name: "next_row_id".into(),
                    })?
                },
                file_size_bytes: if parameterized {
                    match params.get(3) {
                        Some(value) => value.parse().map_err(|_| RockLakeError::MissingParam {
                            name: "file_size_bytes".into(),
                        })?,
                        None => 0,
                    }
                } else {
                    literal_u64(&literals, 3).unwrap_or(0)
                },
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertFileColumnStats => {
            let parameterized = _sql.contains('$');
            let mut ops = Vec::new();
            if let Some(rows) = parse_insert_rows_map(_sql, params)
                .filter(|rows| rows.iter().any(|row| !row.is_empty()))
            {
                for row in rows {
                    let null_count = row_map_u64(&row, "null_count");
                    ops.push(BufferedOp::InsertFileColumnStats {
                        data_file_id: required_row_u64_value(&row, &["data_file_id"])?,
                        table_id: required_row_u64_value(&row, &["table_id"])?,
                        column_id: required_row_u64_value(&row, &["column_id"])?,
                        contains_null: null_count.is_some_and(|count| count > 0),
                        column_size_bytes: row_map_u64(&row, "column_size_bytes"),
                        value_count: row_map_u64(&row, "value_count"),
                        null_count,
                        min_value: row_map_string(&row, "min_value"),
                        max_value: row_map_string(&row, "max_value"),
                        contains_nan: row_map_bool(&row, "contains_nan").unwrap_or(false),
                        extra_stats: row_map_string(&row, "extra_stats"),
                        min_is_exact: row_map_bool(&row, "min_is_exact"),
                        max_is_exact: row_map_bool(&row, "max_is_exact"),
                    });
                }
            } else if parameterized {
                let null_count = required_param_u64(params, 5, "null_count")?;
                ops.push(BufferedOp::InsertFileColumnStats {
                    data_file_id: required_param_u64(params, 0, "data_file_id")?,
                    table_id: required_param_u64(params, 1, "table_id")?,
                    column_id: required_param_u64(params, 2, "column_id")?,
                    contains_null: null_count > 0,
                    column_size_bytes: Some(required_param_u64(params, 3, "column_size_bytes")?),
                    value_count: Some(required_param_u64(params, 4, "value_count")?),
                    null_count: Some(null_count),
                    min_value: params.get_optional_string(6),
                    max_value: params.get_optional_string(7),
                    contains_nan: params
                        .get(8)
                        .map(|value| matches!(value, "t" | "true" | "1" | "TRUE"))
                        .unwrap_or(false),
                    extra_stats: params.get_optional_string(9),
                    min_is_exact: params
                        .get(10)
                        .map(|value| matches!(value, "t" | "true" | "1" | "TRUE")),
                    max_is_exact: params
                        .get(11)
                        .map(|value| matches!(value, "t" | "true" | "1" | "TRUE")),
                });
            } else {
                for literals in literal_insert_rows(_sql) {
                    let null_count =
                        literal_u64(&literals, 5).ok_or_else(|| RockLakeError::MissingParam {
                            name: "null_count".into(),
                        })?;
                    ops.push(BufferedOp::InsertFileColumnStats {
                        data_file_id: literal_u64(&literals, 0).ok_or_else(|| {
                            RockLakeError::MissingParam {
                                name: "data_file_id".into(),
                            }
                        })?,
                        table_id: literal_u64(&literals, 1).ok_or_else(|| {
                            RockLakeError::MissingParam {
                                name: "table_id".into(),
                            }
                        })?,
                        column_id: literal_u64(&literals, 2).ok_or_else(|| {
                            RockLakeError::MissingParam {
                                name: "column_id".into(),
                            }
                        })?,
                        contains_null: null_count > 0,
                        column_size_bytes: Some(literal_u64(&literals, 3).ok_or_else(|| {
                            RockLakeError::MissingParam {
                                name: "column_size_bytes".into(),
                            }
                        })?),
                        value_count: Some(literal_u64(&literals, 4).ok_or_else(|| {
                            RockLakeError::MissingParam {
                                name: "value_count".into(),
                            }
                        })?),
                        null_count: Some(null_count),
                        min_value: literal_string(&literals, 6),
                        max_value: literal_string(&literals, 7),
                        contains_nan: literal_bool(&literals, 8).unwrap_or(false),
                        extra_stats: literal_string(&literals, 9),
                        min_is_exact: literal_bool(&literals, 10),
                        max_is_exact: literal_bool(&literals, 11),
                    });
                }
            }
            let row_count = ops.len();
            if session.in_transaction {
                for op in ops {
                    session.pending_txn.push(op)?;
                }
            } else {
                execute_commit(ops, store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "INSERT 0 {row_count}"
            )))])
        }
        StatementKind::InsertTableColumnStats => {
            let literal_rows = literal_insert_rows(_sql);
            let parameterized = _sql.contains('$');
            let mut ops = Vec::new();
            if let Some(rows) = parse_insert_rows_map(_sql, params)
                .filter(|rows| rows.iter().any(|row| !row.is_empty()))
            {
                for row in rows {
                    ops.push(BufferedOp::InsertTableColumnStats {
                        table_id: required_row_u64_value(&row, &["table_id"])?,
                        column_id: required_row_u64_value(&row, &["column_id"])?,
                        contains_null: row_map_bool(&row, "contains_null").unwrap_or(false),
                        contains_nan: row_map_bool(&row, "contains_nan"),
                        min_value: row_map_string(&row, "min_value"),
                        max_value: row_map_string(&row, "max_value"),
                        extra_stats: row_map_string(&row, "extra_stats"),
                        min_is_exact: row_map_optional_bool(&row, "min_is_exact"),
                        max_is_exact: row_map_optional_bool(&row, "max_is_exact"),
                    });
                }
            } else if parameterized || literal_rows.is_empty() {
                let literals = literal_rows.into_iter().next().unwrap_or_default();
                ops.push(BufferedOp::InsertTableColumnStats {
                    table_id: if parameterized {
                        required_param_u64(params, 0, "table_id")?
                    } else {
                        literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                            name: "table_id".into(),
                        })?
                    },
                    column_id: if parameterized {
                        required_param_u64(params, 1, "column_id")?
                    } else {
                        literal_u64(&literals, 1).ok_or_else(|| RockLakeError::MissingParam {
                            name: "column_id".into(),
                        })?
                    },
                    contains_null: if parameterized {
                        required_param_bool(params, 2, "contains_null")?
                    } else {
                        literal_bool(&literals, 2).ok_or_else(|| RockLakeError::MissingParam {
                            name: "contains_null".into(),
                        })?
                    },
                    contains_nan: if parameterized {
                        Some(required_param_bool(params, 3, "contains_nan")?)
                    } else {
                        literal_bool(&literals, 3)
                    },
                    min_value: params
                        .get_optional_string(4)
                        .or_else(|| literal_string(&literals, 4)),
                    max_value: params
                        .get_optional_string(5)
                        .or_else(|| literal_string(&literals, 5)),
                    extra_stats: params
                        .get_optional_string(6)
                        .or_else(|| literal_string(&literals, 6)),
                    min_is_exact: params
                        .get(7)
                        .map(|value| Some(matches!(value, "t" | "true" | "1" | "TRUE"))),
                    max_is_exact: params
                        .get(8)
                        .map(|value| Some(matches!(value, "t" | "true" | "1" | "TRUE"))),
                });
            } else {
                for literals in literal_rows {
                    ops.push(BufferedOp::InsertTableColumnStats {
                        table_id: literal_u64(&literals, 0).unwrap_or(0),
                        column_id: literal_u64(&literals, 1).unwrap_or(0),
                        contains_null: literal_bool(&literals, 2).unwrap_or(false),
                        contains_nan: literal_bool(&literals, 3),
                        min_value: literal_string(&literals, 4),
                        max_value: literal_string(&literals, 5),
                        extra_stats: literal_string(&literals, 6),
                        min_is_exact: literal_bool(&literals, 7).map(Some),
                        max_is_exact: literal_bool(&literals, 8).map(Some),
                    });
                }
            }
            let row_count = ops.len();
            if session.in_transaction {
                for op in ops {
                    session.pending_txn.push(op)?;
                }
            } else {
                execute_commit(ops, store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "INSERT 0 {row_count}"
            )))])
        }
        StatementKind::UpdateTableColumnStats => {
            let ops = update_table_column_stats(_sql, params, stable_ducklake_v11(store).await?)?;
            let row_count = ops.len();
            if session.in_transaction {
                for op in ops {
                    session.pending_txn.push(op)?;
                }
            } else if !ops.is_empty() {
                execute_commit(ops, store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "UPDATE {row_count}"
            )))])
        }
        StatementKind::InsertMetadata => {
            let parameterized = _sql.contains('$');
            let literals = literal_insert_values(_sql);
            let op = BufferedOp::InsertMetadata {
                key: if parameterized {
                    required_param_string(params, 0, "key")?
                } else {
                    literal_string(&literals, 0)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam { name: "key".into() })?
                },
                value: if parameterized {
                    required_param_string(params, 1, "value")?
                } else {
                    literal_string(&literals, 1).ok_or_else(|| RockLakeError::MissingParam {
                        name: "value".into(),
                    })?
                },
                scope: if parameterized {
                    params.get_optional_string(2)
                } else {
                    literal_string(&literals, 2)
                },
                scope_id: if parameterized {
                    params.get_u64(3).ok()
                } else {
                    literal_u64(&literals, 3)
                },
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::UpdateMetadata => {
            let lower = _sql.to_ascii_lowercase();
            let staging_version_update = session.ducklake_v11_migration_in_progress
                && lower.contains("ducklake_metadata")
                && lower.contains("update_data_")
                && lower.contains("__page_id_string")
                && lower.contains("ctid");
            let key = if staging_version_update {
                Some("version".to_string())
            } else {
                resolve_comparison_string(_sql, "key", params)
            };
            let value = if staging_version_update {
                Some("1.1".to_string())
            } else {
                resolve_assignment_string(_sql, "value", params)
            };
            if key.as_deref() != Some("version") || value.as_deref() != Some("1.1") {
                return Err(RockLakeError::Unsupported(
                    "only the DuckLake 1.0-to-1.1 version update is supported".into(),
                ));
            }
            match current_ducklake_version(store).await?.as_deref() {
                Some("1.0") => {
                    let op = BufferedOp::MigrateDuckLakeV11;
                    if session.in_transaction {
                        session.pending_txn.push(op)?;
                    } else {
                        execute_commit(vec![op], store, notify_manager).await?;
                    }
                }
                Some("1.1") => {}
                Some(version) => {
                    return Err(RockLakeError::Unsupported(format!(
                        "cannot migrate DuckLake catalog version {version} to 1.1"
                    )))
                }
                None => {
                    return Err(RockLakeError::Unsupported(
                        "DuckLake catalog has no version marker to migrate".into(),
                    ))
                }
            }
            Ok(vec![Response::Execution(Tag::new("UPDATE 1"))])
        }
        StatementKind::DuckLakeV11MigrationStep => {
            match current_ducklake_version(store).await?.as_deref() {
                Some("1.0") => session.ducklake_v11_migration_in_progress = true,
                Some("1.1") => {}
                Some(version) => {
                    return Err(RockLakeError::Unsupported(format!(
                    "DuckLake v1.1 migration steps are incompatible with catalog version {version}"
                )))
                }
                None => {
                    return Err(RockLakeError::Unsupported(
                        "DuckLake migration requires a version marker".into(),
                    ))
                }
            }
            Ok(vec![Response::Execution(Tag::new("OK"))])
        }
        StatementKind::InsertSchemaVersions => {
            let literal_rows = literal_insert_rows(_sql);
            let parameterized = _sql.contains('$');
            let mut ops = Vec::new();
            if parameterized || literal_rows.is_empty() {
                let literals = literal_rows.into_iter().next().unwrap_or_default();
                ops.push(BufferedOp::InsertSchemaVersions {
                    begin_snapshot: if parameterized {
                        required_param_u64(params, 0, "begin_snapshot")?
                    } else {
                        literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                            name: "begin_snapshot".into(),
                        })?
                    },
                    schema_version: if parameterized {
                        required_param_u64(params, 1, "schema_version")?
                    } else {
                        literal_u64(&literals, 1).ok_or_else(|| RockLakeError::MissingParam {
                            name: "schema_version".into(),
                        })?
                    },
                    table_id: if parameterized {
                        required_param_u64(params, 2, "table_id")?
                    } else {
                        literal_u64(&literals, 2).ok_or_else(|| RockLakeError::MissingParam {
                            name: "table_id".into(),
                        })?
                    },
                });
            } else {
                for literals in literal_rows {
                    ops.push(BufferedOp::InsertSchemaVersions {
                        begin_snapshot: literal_u64(&literals, 0).unwrap_or(0),
                        schema_version: literal_u64(&literals, 1).unwrap_or(0),
                        table_id: literal_u64(&literals, 2).unwrap_or(0),
                    });
                }
            }
            let row_count = ops.len();
            if session.in_transaction {
                for op in ops {
                    session.pending_txn.push(op)?;
                }
            } else {
                execute_commit(ops, store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "INSERT 0 {row_count}"
            )))])
        }
        StatementKind::InsertInlinedDataTables => {
            let literal_rows = literal_insert_rows(_sql);
            let parameterized = _sql.contains('$');
            let mut ops = Vec::new();
            if parameterized || literal_rows.is_empty() {
                let literals = literal_rows.into_iter().next().unwrap_or_default();
                ops.push(BufferedOp::InsertInlinedDataTables {
                    table_id: if parameterized {
                        required_param_u64(params, 0, "table_id")?
                    } else {
                        literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                            name: "table_id".into(),
                        })?
                    },
                    table_name: if parameterized {
                        required_param_string(params, 1, "table_name")?
                    } else {
                        literal_string(&literals, 1)
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| RockLakeError::MissingParam {
                                name: "table_name".into(),
                            })?
                    },
                    schema_version: if parameterized {
                        required_param_u64(params, 2, "schema_version")?
                    } else {
                        literal_u64(&literals, 2).ok_or_else(|| RockLakeError::MissingParam {
                            name: "schema_version".into(),
                        })?
                    },
                });
            } else {
                for literals in literal_rows {
                    ops.push(BufferedOp::InsertInlinedDataTables {
                        table_id: literal_u64(&literals, 0).unwrap_or(0),
                        table_name: literal_string(&literals, 1).unwrap_or_default(),
                        schema_version: literal_u64(&literals, 2).unwrap_or(0),
                    });
                }
            }
            let row_count = ops.len();
            if session.in_transaction {
                for op in ops {
                    session.pending_txn.push(op)?;
                }
            } else {
                execute_commit(ops, store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "INSERT 0 {row_count}"
            )))])
        }
        StatementKind::InsertView => {
            let op = if let Some(rows) = parse_insert_rows_map(_sql, params) {
                let row = rows.first().ok_or_else(|| RockLakeError::MissingParam {
                    name: "view_name".into(),
                })?;
                BufferedOp::InsertView {
                    schema_id: required_row_u64_value(row, &["schema_id"])?,
                    view_name: required_row_string_value(row, &["view_name"])?,
                    sql: required_row_string_value(row, &["sql", "view_definition"])?,
                    view_uuid: row_map_string(row, "view_uuid"),
                    dialect: row_map_string(row, "dialect"),
                    column_aliases: row_map_string(row, "column_aliases"),
                }
            } else {
                let parameterized = _sql.contains('$');
                let literals = literal_insert_values(_sql);
                BufferedOp::InsertView {
                    schema_id: if parameterized {
                        required_param_u64(params, 0, "schema_id")?
                    } else {
                        literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                            name: "schema_id".into(),
                        })?
                    },
                    view_name: if parameterized {
                        required_param_string(params, 1, "view_name")?
                    } else {
                        literal_string(&literals, 1)
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| RockLakeError::MissingParam {
                                name: "view_name".into(),
                            })?
                    },
                    sql: if parameterized {
                        required_param_string(params, 2, "sql")?
                    } else {
                        literal_string(&literals, 2)
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| RockLakeError::MissingParam { name: "sql".into() })?
                    },
                    view_uuid: params.get_optional_string(3),
                    dialect: params.get_optional_string(4),
                    column_aliases: params.get_optional_string(5),
                }
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertMacro => {
            let parameterized = _sql.contains('$');
            let literals = literal_insert_values(_sql);
            let op = BufferedOp::InsertMacro {
                schema_id: if parameterized {
                    required_param_u64(params, 0, "schema_id")?
                } else {
                    literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                        name: "schema_id".into(),
                    })?
                },
                macro_name: if parameterized {
                    required_param_string(params, 1, "macro_name")?
                } else {
                    literal_string(&literals, 1)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "macro_name".into(),
                        })?
                },
                macro_type: if parameterized {
                    required_param_string(params, 2, "macro_type")?
                } else {
                    literal_string(&literals, 2)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "macro_type".into(),
                        })?
                },
                macro_uuid: params.get_optional_string(3),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertMacroImpl => {
            let parameterized = _sql.contains('$');
            let literals = literal_insert_values(_sql);
            let op = BufferedOp::InsertMacroImpl {
                macro_id: if parameterized {
                    required_param_u64(params, 0, "macro_id")?
                } else {
                    literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                        name: "macro_id".into(),
                    })?
                },
                sql: if parameterized {
                    required_param_string(params, 1, "sql")?
                } else {
                    literal_string(&literals, 1)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam { name: "sql".into() })?
                },
                dialect: params.get_optional_string(2),
                impl_type: params.get_optional_string(3),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertMacroParameters => {
            let parameterized = _sql.contains('$');
            let literals = literal_insert_values(_sql);
            let op = BufferedOp::InsertMacroParams {
                macro_id: if parameterized {
                    required_param_u64(params, 0, "macro_id")?
                } else {
                    literal_u64(&literals, 0).ok_or_else(|| RockLakeError::MissingParam {
                        name: "macro_id".into(),
                    })?
                },
                impl_id: if parameterized {
                    required_param_u64(params, 1, "impl_id")?
                } else {
                    literal_u64(&literals, 1).ok_or_else(|| RockLakeError::MissingParam {
                        name: "impl_id".into(),
                    })?
                },
                column_id: if parameterized {
                    required_param_u64(params, 2, "column_id")?
                } else {
                    literal_u64(&literals, 2).ok_or_else(|| RockLakeError::MissingParam {
                        name: "column_id".into(),
                    })?
                },
                parameter_name: if parameterized {
                    required_param_string(params, 3, "parameter_name")?
                } else {
                    literal_string(&literals, 3)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "parameter_name".into(),
                        })?
                },
                parameter_type: if parameterized {
                    required_param_string(params, 4, "parameter_type")?
                } else {
                    literal_string(&literals, 4)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "parameter_type".into(),
                        })?
                },
                default_value: params.get_optional_string(5),
                default_value_type: params.get_optional_string(6),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }

        StatementKind::UpdateEndSnapshot(ref table_name) => {
            if table_name == "ducklake_view_column_tag" {
                if !updates_column(_sql, "end_snapshot") {
                    let lower = _sql.to_ascii_lowercase();
                    if lower.starts_with("with overwritten_tags")
                        && lower.contains("update")
                        && lower.contains("ducklake_view_column_tag")
                        && lower.contains("set end_snapshot")
                        && lower.contains("from overwritten_tags")
                    {
                        // DuckDB's replacement INSERT retires the matching live tag itself.
                        return Ok(vec![Response::Execution(Tag::new("UPDATE 0"))]);
                    }
                    return Err(RockLakeError::Unsupported(
                        "only end_snapshot updates are supported for ducklake_view_column_tag"
                            .into(),
                    ));
                }
                let op = BufferedOp::EndViewColumnTag {
                    view_id: resolve_comparison_u64(_sql, "view_id", params).ok_or_else(|| {
                        RockLakeError::MissingParam {
                            name: "view_id".into(),
                        }
                    })?,
                    column_name: resolve_comparison_string(_sql, "column_name", params)
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "column_name".into(),
                        })?,
                    tag_key: resolve_comparison_string(_sql, "key", params)
                        .ok_or_else(|| RockLakeError::MissingParam { name: "key".into() })?,
                    begin_snapshot: resolve_comparison_u64(_sql, "begin_snapshot", params)
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "begin_snapshot".into(),
                        })?,
                };
                submit_ops(vec![op], session, store, notify_manager).await?;
                return Ok(vec![Response::Execution(Tag::new("UPDATE 1"))]);
            }
            let mut entity_id = params.get_u64(1).unwrap_or(0);
            let begin_snapshot = params.get_u64(2).unwrap_or(0);
            let mut end_snapshot = params.get_u64(0).unwrap_or(0);
            let mut where_column = None;

            // DROP COLUMN uses a dropped_cols(tid, cid) CTE for both the
            // column and its tags. Neither WHERE clause contains a literal
            // column_id, so resolve cid from the CTE in both cases.
            let dropped_column_ids = if matches!(
                table_name.as_str(),
                "ducklake_column" | "ducklake_column_tag"
            ) {
                ducklake_dropped_column_ids(_sql)
            } else {
                None
            };
            if let Some((_, dropped_column_id)) = dropped_column_ids {
                entity_id = dropped_column_id;
                where_column = Some("column_id".to_string());
            }

            if entity_id == 0 {
                if let Some(extracted) = extract_id_from_where_clause(_sql) {
                    entity_id = extracted.value;
                    where_column = Some(extracted.column);
                }
            } else if where_column.is_none() {
                if let Some(extracted) = extract_id_from_where_clause(_sql) {
                    where_column = Some(extracted.column);
                }
            }

            if entity_id == 0 {
                let where_key = if table_name == "ducklake_column" {
                    "column_id"
                } else {
                    "table_id"
                };
                if let Some(resolved) = resolve_comparison_u64(_sql, where_key, params) {
                    entity_id = resolved;
                    if where_column.is_none() {
                        where_column = Some(where_key.to_string());
                    }
                }
            }

            if end_snapshot == 0 {
                if let Some(val) = literal_assignment_u64(_sql, "end_snapshot") {
                    end_snapshot = val;
                }
            }

            if entity_id == 0 || end_snapshot == 0 {
                return Err(RockLakeError::MissingParam {
                    name: "entity_id/end_snapshot".into(),
                });
            }

            let op = BufferedOp::UpdateEndSnapshot {
                table_name: table_name.clone(),
                entity_id,
                table_id: resolve_comparison_u64(_sql, "table_id", params)
                    .or_else(|| dropped_column_ids.map(|(table_id, _)| table_id)),
                begin_snapshot,
                end_snapshot,
                where_column,
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("UPDATE 1"))])
        }
        StatementKind::UpdateTableStats => {
            let op = if let (
                Some(table_id),
                Some(record_count),
                Some(file_size_bytes),
                Some(next_row_id),
            ) = (
                literal_assignment_u64(_sql, "table_id"),
                literal_assignment_u64(_sql, "record_count"),
                literal_assignment_u64(_sql, "file_size_bytes"),
                literal_assignment_u64(_sql, "next_row_id"),
            ) {
                BufferedOp::SetTableStats {
                    table_id,
                    record_count,
                    file_size_bytes,
                    next_row_id,
                }
            } else if _sql
                .to_ascii_lowercase()
                .contains("sum(data.file_size_bytes)")
            {
                let inner = literal_comparison_u64(_sql, "table_id");
                let outer = _sql
                    .to_ascii_lowercase()
                    .rfind("where")
                    .and_then(|idx| literal_comparison_u64(&_sql[idx..], "table_id"));
                let table_id = inner.filter(|id| Some(*id) == outer).ok_or_else(|| {
                    RockLakeError::ValueDecode("mismatched table IDs in file size update".into())
                })?;
                BufferedOp::RecomputeTableFileSize { table_id }
            } else {
                BufferedOp::UpdateTableStats {
                    table_id: required_param_u64(params, 1, "table_id")?,
                    row_count_delta: required_param_i64(params, 0, "row_count_delta")?,
                }
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("UPDATE 1"))])
        }

        // ─── Inlined Data DDL/DML ──────────────────────────────────────
        StatementKind::CreateInlinedTable => {
            // DuckDB 2.0's postgres scanner refreshes pg_class after CREATE
            // TABLE, before DuckLake's first snapshot has been committed.
            // Make only the relations already declared here discoverable.
            if let Ok(statements) = Parser::parse_sql(&PostgreSqlDialect {}, _sql) {
                if let Some(Statement::CreateTable(create)) = statements.first() {
                    let qualified = create.name.to_string();
                    let table = qualified
                        .rsplit('.')
                        .next()
                        .unwrap_or(&qualified)
                        .trim_matches('"')
                        .to_ascii_lowercase();
                    if crate::schema_registry::fields_for_table(&table).is_some()
                        || parse_inlined_delete_table_id(&table).is_some()
                    {
                        store.lock().await.note_bootstrap_relation(&table);
                    }
                }
            }
            Ok(vec![Response::Execution(Tag::new("CREATE TABLE"))])
        }
        StatementKind::InsertInlinedRow => {
            let table_name = ducklake_insert_table_name(_sql).unwrap_or_default();
            if let Some(table_id) = parse_inlined_delete_table_id(&table_name) {
                let ops = literal_insert_rows(_sql)
                    .into_iter()
                    .filter_map(|row| {
                        Some(BufferedOp::InsertInlinedDelete {
                            table_id,
                            data_file_id: literal_u64(&row, 0)?,
                            row_id: literal_u64(&row, 1)?,
                        })
                    })
                    .collect();
                let row_count = submit_ops(ops, session, store, notify_manager).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            if matches!(
                table_name.as_str(),
                "ducklake_column_mapping"
                    | "ducklake_name_mapping"
                    | "ducklake_partition_info"
                    | "ducklake_sort_info"
                    | "ducklake_files_scheduled_for_deletion"
                    | "ducklake_partition_column"
                    | "ducklake_file_partition_value"
                    | "ducklake_sort_expression"
                    | "ducklake_view_column_tag"
            ) {
                let rows = parse_insert_rows_map(_sql, params).ok_or_else(|| {
                    RockLakeError::Unsupported("could not parse DuckLake metadata insert".into())
                })?;
                let row_count = submit_ops(
                    vec![BufferedOp::InsertDuckLakeRows {
                        table_name: table_name.clone(),
                        rows,
                    }],
                    session,
                    store,
                    notify_manager,
                )
                .await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            let rows = literal_insert_rows(_sql);
            let row_count = rows.len();
            let op = BufferedOp::InsertInlinedRow { table_name, rows };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "INSERT 0 {row_count}"
            )))])
        }
        StatementKind::UpdateInlinedRowEndSnapshot => {
            let table_name = inlined_table_name_from_sql(_sql).unwrap_or_default();
            let row_ids = literal_insert_rows(_sql)
                .into_iter()
                .filter_map(|row| literal_u64(&row, 0))
                .collect::<Vec<_>>();
            let row_count = row_ids.len();
            if !table_name.is_empty() && !row_ids.is_empty() {
                let op = BufferedOp::DeleteInlinedRows {
                    table_name,
                    row_ids,
                };
                if session.in_transaction {
                    session.pending_txn.push(op)?;
                } else {
                    execute_commit(vec![op], store, notify_manager).await?;
                }
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "UPDATE {row_count}"
            )))])
        }

        // ─── Virtual Catalog SQL Tables ────────────────────────────────
        // SELECT * FROM rocklake_catalog.{table_name}: read-only introspection.
        // Mutations are rejected with SQLSTATE 25006.
        StatementKind::VirtualCatalogScan { ref table_name } => {
            execute_virtual_catalog_scan(table_name, _sql, store).await
        }

        // DELETE FROM "public".ducklake_inlined_data_<tid>_<sv> WHERE ctid IN (...)
        // Issued by DuckLake CHECKPOINT after flushing inlined rows to Parquet.
        StatementKind::DeleteInlinedDataRows { ref table_name } => {
            let mut row_ids = row_ids_from_ctid_sql(_sql);
            if row_ids.is_empty() {
                let stable_1_1 = stable_ducklake_v11(store).await?;
                let begin_snapshot_column = if stable_1_1 {
                    "_ducklake_begin_snapshot"
                } else {
                    "begin_snapshot"
                };
                if let Some(snapshot_threshold) =
                    resolve_comparison_u64(_sql, begin_snapshot_column, params)
                {
                    if let Some((table_id, _)) = parse_inlined_table_ids(table_name) {
                        let reader = { store.lock().await.read_latest() };
                        if let Ok(all_inlined) = reader.list_inlined_inserts(table_id).await {
                            for row in all_inlined {
                                if row.begin_snapshot <= snapshot_threshold {
                                    row_ids.push(row.row_id);
                                }
                            }
                        }
                    }
                }
            }
            let row_count = row_ids.len();
            if !row_ids.is_empty() {
                let op = BufferedOp::DeleteInlinedRows {
                    table_name: table_name.clone(),
                    row_ids,
                };
                if session.in_transaction {
                    session.pending_txn.push(op)?;
                } else {
                    execute_commit(vec![op], store, notify_manager).await?;
                }
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "DELETE {row_count}"
            )))])
        }

        // DELETE FROM "public".ducklake_data_file WHERE data_file_id IN (...) or similar
        // Issued by DuckLake CHECKPOINT for garbage collection of old data files.
        StatementKind::DeleteDuckLakeCatalogRows { ref table_name } => {
            if let Some(table_id) = parse_inlined_delete_table_id(table_name) {
                let selected_rows = std::mem::take(&mut session.inlined_delete_ctid_rows);
                let ops = row_ids_from_ctid_sql(_sql)
                    .into_iter()
                    .filter_map(|row_id| selected_rows.get(row_id as usize))
                    .map(|row| BufferedOp::DeleteInlinedDelete {
                        table_id,
                        data_file_id: row.data_file_id,
                        row_id: row.row_id,
                    })
                    .collect::<Vec<_>>();
                let count = submit_ops(ops, session, store, notify_manager).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "DELETE {count}"
                )))]);
            }
            if table_name == "ducklake_metadata" && _sql.to_ascii_lowercase().contains("ctid") {
                use rocklake_core::keys::MetadataScope;
                let selected_rows = std::mem::take(&mut session.metadata_ctid_rows);
                let ops = row_ids_from_ctid_sql(_sql)
                    .into_iter()
                    .filter_map(|row_id| selected_rows.get(row_id as usize))
                    .map(|row| BufferedOp::DeleteMetadata {
                        key: row.key.clone(),
                        scope: match row.scope.as_deref() {
                            Some("schema") => MetadataScope::Schema,
                            Some("table") => MetadataScope::Table,
                            _ => MetadataScope::Global,
                        },
                        scope_id: row.scope_id.unwrap_or(0),
                    })
                    .collect();
                let count = submit_ops(ops, session, store, notify_manager).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "DELETE {count}"
                )))]);
            }
            if table_name == "ducklake_view_column_tag" {
                let op = BufferedOp::EndViewColumnTag {
                    view_id: resolve_comparison_u64(_sql, "view_id", params).ok_or_else(|| {
                        RockLakeError::MissingParam {
                            name: "view_id".into(),
                        }
                    })?,
                    column_name: resolve_comparison_string(_sql, "column_name", params)
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "column_name".into(),
                        })?,
                    tag_key: resolve_comparison_string(_sql, "key", params)
                        .ok_or_else(|| RockLakeError::MissingParam { name: "key".into() })?,
                    begin_snapshot: resolve_comparison_u64(_sql, "begin_snapshot", params)
                        .ok_or_else(|| RockLakeError::MissingParam {
                            name: "begin_snapshot".into(),
                        })?,
                };
                submit_ops(vec![op], session, store, notify_manager).await?;
                return Ok(vec![Response::Execution(Tag::new("DELETE 1"))]);
            }
            if table_name == "ducklake_inlined_data_tables" {
                let table_ids = inlined_data_table_ids_from_ctid_sql(_sql);
                let row_count = table_ids.len();
                if session.in_transaction {
                    for (table_id, schema_version) in table_ids {
                        session
                            .pending_txn
                            .push(BufferedOp::DeleteInlinedDataTable {
                                table_id,
                                schema_version,
                            })?;
                    }
                } else if row_count > 0 {
                    let ops = table_ids
                        .into_iter()
                        .map(
                            |(table_id, schema_version)| BufferedOp::DeleteInlinedDataTable {
                                table_id,
                                schema_version,
                            },
                        )
                        .collect();
                    execute_commit(ops, store, notify_manager).await?;
                }
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "DELETE {row_count}"
                )))]);
            }
            if table_name == "ducklake_files_scheduled_for_deletion"
                && _sql.to_ascii_lowercase().contains("ctid")
            {
                let file_ids = row_ids_from_ctid_sql(_sql);
                let reader = { store.lock().await.read_latest() };
                let rows = reader
                    .list_files_scheduled_for_deletion()
                    .await
                    .map_err(RockLakeError::from)?;
                let ops: Vec<_> = rows
                    .into_iter()
                    .filter(|row| file_ids.contains(&row.data_file_id))
                    .map(|row| BufferedOp::DeleteScheduledFile {
                        schedule_start: row.schedule_start,
                        data_file_id: row.data_file_id,
                    })
                    .collect();
                let count = submit_ops(ops, session, store, notify_manager).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "DELETE {count}"
                )))]);
            }
            let file_ids = file_ids_from_where_sql(_sql);
            let row_count = file_ids.len();
            if !file_ids.is_empty() {
                let op = BufferedOp::DeleteDuckLakeCatalogRows {
                    table_name: table_name.clone(),
                    file_ids,
                };
                if session.in_transaction {
                    session.pending_txn.push(op)?;
                } else {
                    execute_commit(vec![op], store, notify_manager).await?;
                }
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "DELETE {row_count}"
            )))])
        }

        // INSERT/UPDATE/DELETE against rocklake_catalog.* → SQLSTATE 25006.
        StatementKind::VirtualCatalogMutation { .. } => Err(RockLakeError::ReadOnlyReplica),

        // ─── v0.18: DuckLake Standard Interface ────────────────────────────
        StatementKind::TableChanges {
            ref table_ref,
            start_snapshot,
            end_snapshot,
        } => {
            // When the SQL uses $N placeholders (e.g. table_changes($1, $2, $3))
            // the classifier stores the literal "$1" / 0 / u64::MAX fallbacks.
            // Resolve the actual values from the runtime params in that case.
            let (resolved_ref, resolved_start, resolved_end) =
                if let Some(rest) = table_ref.strip_prefix('$') {
                    let pidx = rest.parse::<usize>().unwrap_or(1) - 1;
                    let r = params
                        .get_string(pidx)
                        .unwrap_or_else(|_| table_ref.clone());
                    let s = params.get_u64(1).unwrap_or(start_snapshot);
                    let e = params.get_u64(2).unwrap_or(end_snapshot);
                    (r, s, e)
                } else {
                    (table_ref.clone(), start_snapshot, end_snapshot)
                };
            execute_table_changes(&resolved_ref, resolved_start, resolved_end, store).await
        }
        StatementKind::NextRowidRange {
            ref table_ref,
            count,
        } => execute_next_rowid_range(table_ref, count, store).await,
        StatementKind::HoldSnapshot {
            min_snapshot_id,
            ref consumer_id,
            ttl_seconds,
        } => execute_hold_snapshot(min_snapshot_id, consumer_id, ttl_seconds, store).await,
        StatementKind::ReleaseSnapshot { ref consumer_id } => {
            execute_release_snapshot(consumer_id, store).await
        }
        StatementKind::Listen { ref channel } => {
            session.subscriptions.listen(channel, notify_manager).await;
            Ok(vec![Response::Execution(Tag::new("LISTEN"))])
        }
        StatementKind::Unlisten { ref channel } => {
            session.subscriptions.unlisten(channel);
            Ok(vec![Response::Execution(Tag::new("UNLISTEN"))])
        }
        StatementKind::CreateExtensionTable {
            ref schema_name,
            ref table_name,
        } => {
            execute_create_extension_table(schema_name, table_name, store, extension_schemas).await
        }
        StatementKind::InsertExtensionRow {
            ref schema_name,
            ref table_name,
            ref columns,
            ..
        } => {
            execute_insert_extension_row(
                schema_name,
                table_name,
                columns,
                params,
                store,
                extension_schemas,
            )
            .await
        }
        StatementKind::SelectExtensionTable {
            ref schema_name,
            ref table_name,
        } => {
            execute_select_extension_table(schema_name, table_name, store, extension_schemas).await
        }
        StatementKind::DeleteExtensionRows {
            ref schema_name,
            ref table_name,
        } => execute_delete_extension_rows(schema_name, table_name, store, extension_schemas).await,

        // ─── COPY Protocol (DuckDB 1.5+ binary COPY) ───────────────────
        StatementKind::CopyFromStdin { ref table } => {
            if table.starts_with("update_data_") && !session.ducklake_v11_migration_in_progress {
                return Err(RockLakeError::Unsupported(format!(
                    "COPY into migration staging table {table} outside DuckLake v1.1 migration"
                )));
            }
            let inlined_column_types =
                if let Some((table_id, _)) = catalog::parse_inlined_table_ids(table) {
                    let reader = { store.lock().await.read_latest() };
                    let (_, columns) = reader
                        .describe_table(table_id)
                        .await
                        .map_err(RockLakeError::from)?
                        .ok_or_else(|| RockLakeError::NotFound(table.clone()))?;
                    Some(columns.into_iter().map(|column| column.data_type).collect())
                } else {
                    None
                };
            // Record which table is being loaded so on_copy_data / on_copy_done
            // can accumulate and parse the binary stream correctly.
            session.pending_copy = Some(CopyAccumulator {
                table: table.clone(),
                data: Vec::new(),
                inlined_column_types,
            });
            let format = if table.starts_with("update_data_") {
                0
            } else {
                1
            };
            Ok(vec![Response::CopyIn(CopyResponse::new(format, 0, vec![]))])
        }
        StatementKind::CopyToStdout { ref query } => {
            // COPY TO STDOUT requires streaming binary data through the wire
            // protocol. This needs additional handler-level work to send
            // CopyData messages with the actual query results in binary format.
            //
            // For now, return an unsupported error. DuckDB will receive this
            // error after ATTACH when trying to read catalog data.
            Err(RockLakeError::Unsupported(format!(
                "COPY TO STDOUT not yet implemented: {}",
                query
            )))
        }

        StatementKind::Unsupported(ref desc) => Err(RockLakeError::Unsupported(desc.clone())),
    }
}
