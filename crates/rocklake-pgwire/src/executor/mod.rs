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

use std::sync::Arc;

use pgwire::api::results::{CopyResponse, Response, Tag};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use rocklake_catalog::CatalogStore;
use rocklake_sql::{classify_statement, ParamValues, StatementKind};

use crate::error::RockLakeError;
use crate::notify::NotifyManager;
use crate::session::{BufferedOp, CopyAccumulator, SessionState};

use catalog::{
    execute_commit, execute_next_rowid_range, execute_table_changes, make_column_mapping_response,
    make_column_tags_response, make_columns_response, make_data_files_response,
    make_delete_files_response, make_file_column_stats_response, make_file_ids_response,
    make_file_variant_stats_response, make_files_scheduled_for_deletion_response,
    make_global_table_stats_response, make_inlined_data_tables_response,
    make_inlined_rows_response, make_latest_snapshot_info_response, make_macro_impls_response,
    make_macro_parameters_response, make_macros_response, make_metadata_response,
    make_metadata_table_empty_response, make_name_mapping_response,
    make_partition_columns_response, make_partition_info_response, make_schema_version_response,
    make_schema_versions_response, make_schemas_response, make_snapshot_changes_response,
    make_snapshot_row_response, make_snapshot_stats_changes_response,
    make_sort_expressions_response, make_sort_info_response, make_table_column_stats_response,
    make_table_stats_rows_response_for_sql, make_tables_response, make_tags_response,
    make_views_response, parse_inlined_table_ids,
};
use extension::{
    execute_create_extension_table, execute_delete_extension_rows, execute_insert_extension_row,
    execute_select_extension_table,
};
use helpers::{
    apply_set, get_show_value, get_snapshot_param, make_empty_response, make_false_bool_response,
    make_null_int_response, make_null_text_response, make_pg_catalog_inlined_table_response,
    make_pg_catalog_scan_responses, make_pg_type_response, make_single_int_response,
    make_single_text_response, make_version_with_rds_check_response, require_param_u64,
};
use meta::execute_virtual_catalog_scan;
use session::{execute_hold_snapshot, execute_release_snapshot};

/// Execute a SQL statement against the catalog, returning PG wire responses.
pub async fn execute_sql<'a>(
    sql: &'a str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
    session: &mut SessionState,
    notify_manager: &Arc<NotifyManager>,
    extension_schemas: &Arc<Vec<String>>,
) -> Result<Vec<Response<'a>>, RockLakeError> {
    println!("[SQL] execute_sql: {}", sql);
    let has_delete = sql.to_lowercase().contains("delete");
    let has_ducklake = sql.to_lowercase().contains("ducklake");

    // Detect if this is a batch with DELETE statements
    if has_delete && has_ducklake {
        // Try to split and process manually
        let parts: Vec<&str> = sql.split(';').collect();
        if parts.len() > 1 {
            let mut all_responses = Vec::new();
            for part in parts.iter() {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Try to classify - if it fails, include the error in the result
                match classify_statement(trimmed) {
                    Ok(kind) => {
                        match execute_classified(
                            kind,
                            trimmed,
                            params,
                            store,
                            session,
                            notify_manager,
                            extension_schemas,
                        )
                        .await
                        {
                            Ok(mut responses) => all_responses.append(&mut responses),
                            Err(e) => {
                                return Err(RockLakeError::Unsupported(format!("EXEC_FAIL[{}]", e)))
                            }
                        }
                    }
                    Err(e) => return Err(RockLakeError::Unsupported(format!("CLASS_FAIL[{}]", e))),
                }
            }
            return Ok(all_responses);
        }
    }

    if let Some(statements) = parse_multi_statement_batch(sql) {
        let mut all_responses = Vec::new();
        for statement_sql in statements {
            let kind = classify_statement(&statement_sql)?;
            let mut responses = execute_classified(
                kind,
                &statement_sql,
                params,
                store,
                session,
                notify_manager,
                extension_schemas,
            )
            .await?;
            all_responses.append(&mut responses);
        }
        return Ok(all_responses);
    }

    let kind = classify_statement(sql)?;
    execute_classified(
        kind,
        sql,
        params,
        store,
        session,
        notify_manager,
        extension_schemas,
    )
    .await
}

fn parse_multi_statement_batch(sql: &str) -> Option<Vec<String>> {
    if !sql.contains(';') {
        return None;
    }

    let lower = sql.to_lowercase();
    if lower.contains("copy ") || (lower.contains("pg_namespace") && lower.contains("pg_class")) {
        return None;
    }

    let dialect = PostgreSqlDialect {};
    match Parser::parse_sql(&dialect, sql) {
        Ok(statements) => {
            if statements.len() <= 1 {
                return None;
            }

            let result: Vec<String> = statements
                .into_iter()
                .map(|stmt| stmt.to_string())
                .collect();

            Some(result)
        }
        Err(_) => {
            // Parser failed - if this contains DELETE or has multiple semicolons,
            // try fallback splitting by semicolon
            if lower.contains("delete") || sql.matches(';').count() > 1 {
                // Manual split by semicolon as fallback
                let parts: Vec<String> = sql
                    .split(';')
                    .map(|s| s.to_string())
                    .filter(|s| !s.trim().is_empty())
                    .collect();

                if parts.len() > 1 {
                    return Some(parts);
                }
            }
            None
        }
    }
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

/// Extract file IDs from a DuckLake CHECKPOINT catalog DELETE:
///   `DELETE FROM "public".ducklake_data_file WHERE data_file_id IN (1, 2);`
/// Handles various DuckLake catalog tables that need garbage collection.
fn file_ids_from_where_sql(sql: &str) -> Vec<u64> {
    let mut ids = Vec::new();
    let lower = sql.to_lowercase();

    // Find the WHERE clause
    let Some(where_idx) = lower.find(" where ") else {
        return ids;
    };
    let after_where = &sql[where_idx + 7..];
    let lower_after = after_where.to_lowercase();

    // Pattern 1: data_file_id IN (id1, id2, ...) or delete_file_id IN (...)
    // Look for both " in(" and just "in(" patterns
    let in_patterns = [" in(", "in("];
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
        sqlparser::ast::Expr::Cast { expr, .. } => resolve_expr(expr, params),
        sqlparser::ast::Expr::Identifier(ident) => Some(ident.value.clone()),
        _ => Some(expr.to_string()),
    }
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
    let table_name_normalized = table_name.trim_matches('"');

    let mut cols: Vec<String> = insert
        .columns
        .iter()
        .map(|c| c.value.to_lowercase())
        .collect();

    if cols.is_empty() {
        if table_name_normalized == "ducklake_column_mapping" {
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
        } else if table_name_normalized == "ducklake_sort_info" {
            cols = vec![
                "sort_id".to_string(),
                "table_id".to_string(),
                "begin_snapshot".to_string(),
                "end_snapshot".to_string(),
            ];
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
        } else if table_name_normalized.contains("ducklake_schema") {
            cols = vec![
                "schema_id".to_string(),
                "schema_uuid".to_string(),
                "begin_snapshot".to_string(),
                "end_snapshot".to_string(),
                "schema_name".to_string(),
                "path".to_string(),
                "path_is_relative".to_string(),
            ];
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

fn row_map_string(
    row: &std::collections::HashMap<String, Option<String>>,
    key: &str,
) -> Option<String> {
    row.get(key).and_then(|value| value.clone())
}

fn row_map_u64(row: &std::collections::HashMap<String, Option<String>>, key: &str) -> Option<u64> {
    row.get(key)
        .and_then(|value| value.as_ref().and_then(|text| text.parse::<u64>().ok()))
}

fn row_map_bool(
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

async fn execute_ducklake_column_mapping_insert(
    sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<usize, RockLakeError> {
    let Some(rows) = parse_insert_rows_map(sql, params) else {
        return Ok(0);
    };

    let mut catalog = store.lock().await;
    let mut writer = catalog.begin_write();
    let mut row_count = 0usize;

    for row in rows {
        let table_id = row
            .get("table_id")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<u64>().ok()))
            .unwrap_or(0);
        let mapping_id = row
            .get("mapping_id")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<u64>().ok()));
        let column_id = row
            .get("column_id")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<u64>().ok()));
        let file_column_name = row
            .get("file_column_name")
            .and_then(|value| value.as_deref())
            .or_else(|| row.get("column_name").and_then(|value| value.as_deref()));
        let mapping_type = row
            .get("mapping_type")
            .and_then(|value| value.as_deref())
            .or_else(|| row.get("type").and_then(|value| value.as_deref()));

        writer
            .add_column_mapping(
                table_id,
                mapping_id,
                column_id,
                file_column_name,
                mapping_type,
            )
            .await
            .map_err(RockLakeError::from)?;
        row_count += 1;
    }

    Ok(row_count)
}

async fn execute_ducklake_name_mapping_insert(
    sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<usize, RockLakeError> {
    let Some(rows) = parse_insert_rows_map(sql, params) else {
        return Ok(0);
    };

    let mut catalog = store.lock().await;
    let mut writer = catalog.begin_write();
    let mut row_count = 0usize;

    for row in rows {
        let column_id = row
            .get("column_id")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<u64>().ok()))
            .unwrap_or(0);
        let mapping_id = row
            .get("mapping_id")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<u64>().ok()));
        let name = row
            .get("name")
            .and_then(|value| value.as_deref())
            .or_else(|| row.get("source_name").and_then(|value| value.as_deref()))
            .or_else(|| row.get("field_name").and_then(|value| value.as_deref()))
            .unwrap_or_default();
        let source_name_hash = row
            .get("source_name_hash")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<u64>().ok()));
        let target_field_id = row
            .get("target_field_id")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<u64>().ok()));
        let parent_column = row
            .get("parent_column")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<u64>().ok()));
        let is_partition = row
            .get("is_partition")
            .and_then(|value| value.as_ref().and_then(|s| s.parse::<bool>().ok()));

        writer
            .add_name_mapping(
                mapping_id,
                column_id,
                name,
                source_name_hash,
                target_field_id,
                parent_column,
                is_partition,
            )
            .await
            .map_err(RockLakeError::from)?;
        row_count += 1;
    }

    Ok(row_count)
}

async fn execute_ducklake_partition_info_insert(
    sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<usize, RockLakeError> {
    let Some(rows) = parse_insert_rows_map(sql, params) else {
        return Ok(0);
    };

    let mut catalog = store.lock().await;
    let mut writer = catalog.begin_write();
    let mut row_count = 0usize;

    for row in rows {
        let table_id = row_map_u64(&row, "table_id").unwrap_or(0);
        let partition_id = row_map_u64(&row, "partition_id");
        writer
            .register_partition_info(table_id, partition_id)
            .await
            .map_err(RockLakeError::from)?;
        row_count += 1;
    }

    Ok(row_count)
}

async fn execute_ducklake_sort_info_insert(
    sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<usize, RockLakeError> {
    let Some(rows) = parse_insert_rows_map(sql, params) else {
        return Ok(0);
    };

    let mut catalog = store.lock().await;
    let mut writer = catalog.begin_write();
    let mut row_count = 0usize;

    for row in rows {
        let table_id = row_map_u64(&row, "table_id").unwrap_or(0);
        let sort_id = row_map_u64(&row, "sort_id");
        writer
            .register_sort_info(table_id, sort_id)
            .await
            .map_err(RockLakeError::from)?;
        row_count += 1;
    }

    Ok(row_count)
}

async fn execute_ducklake_files_scheduled_for_deletion_insert(
    sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<usize, RockLakeError> {
    let Some(rows) = parse_insert_rows_map(sql, params) else {
        return Ok(0);
    };

    let mut catalog = store.lock().await;
    let mut writer = catalog.begin_write();
    let mut row_count = 0usize;

    for row in rows {
        let data_file_id = row_map_u64(&row, "data_file_id").unwrap_or(0);
        let path = row_map_string(&row, "path").unwrap_or_default();
        let path_is_relative = row_map_bool(&row, "path_is_relative");
        let schedule_start = row_map_u64(&row, "schedule_start")
            .or_else(|| row_map_u64(&row, "deletion_scheduled_at"))
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });
        let file_type = row_map_string(&row, "file_type");
        writer
            .schedule_file_deletion_with_opts(
                data_file_id,
                &path,
                file_type.as_deref(),
                path_is_relative,
                schedule_start,
            )
            .await
            .map_err(RockLakeError::from)?;
        row_count += 1;
    }

    Ok(row_count)
}

async fn execute_ducklake_partition_column_insert(
    sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<usize, RockLakeError> {
    let Some(rows) = parse_insert_rows_map(sql, params) else {
        return Ok(0);
    };

    let mut catalog = store.lock().await;
    let mut writer = catalog.begin_write();
    let mut row_count = 0usize;

    for row in rows {
        let partition_id = row_map_u64(&row, "partition_id").unwrap_or(0);
        let partition_key_index = row_map_u64(&row, "partition_key_index").unwrap_or(0);
        let column_id = row_map_u64(&row, "column_id").unwrap_or(0);
        let transform = row_map_string(&row, "transform");
        let table_id = row_map_u64(&row, "table_id");
        writer
            .register_partition_column(
                partition_id,
                partition_key_index,
                column_id,
                transform.as_deref(),
                table_id,
            )
            .await
            .map_err(RockLakeError::from)?;
        row_count += 1;
    }

    Ok(row_count)
}

async fn execute_ducklake_sort_expression_insert(
    sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
) -> Result<usize, RockLakeError> {
    let Some(rows) = parse_insert_rows_map(sql, params) else {
        return Ok(0);
    };

    let mut catalog = store.lock().await;
    let mut writer = catalog.begin_write();
    let mut row_count = 0usize;

    for row in rows {
        let sort_id = row_map_u64(&row, "sort_id").unwrap_or(0);
        let sort_key_index = row_map_u64(&row, "sort_key_index").unwrap_or(0);
        let column_id = row_map_u64(&row, "column_id").unwrap_or(0);
        let sort_direction = row_map_string(&row, "sort_direction");
        let null_order = row_map_string(&row, "null_order");
        let table_id = row_map_u64(&row, "table_id");
        let expression = row_map_string(&row, "expression");
        let dialect = row_map_string(&row, "dialect");
        writer
            .register_sort_expression(
                sort_id,
                sort_key_index,
                column_id,
                sort_direction.as_deref(),
                null_order.as_deref(),
                table_id,
                expression.as_deref(),
                dialect.as_deref(),
            )
            .await
            .map_err(RockLakeError::from)?;
        row_count += 1;
    }

    Ok(row_count)
}

fn inlined_table_name_from_sql(sql: &str) -> Option<String> {
    let lower = sql.to_ascii_lowercase();
    let start = lower.find("ducklake_inlined_data_")?;
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

async fn execute_classified<'a>(
    kind: StatementKind,
    _sql: &str,
    params: &ParamValues,
    store: &Arc<tokio::sync::Mutex<CatalogStore>>,
    session: &mut SessionState,
    notify_manager: &Arc<NotifyManager>,
    extension_schemas: &Arc<Vec<String>>,
) -> Result<Vec<Response<'a>>, RockLakeError> {
    match kind {
        // ─── Session / Introspection ───────────────────────────────────
        StatementKind::SelectVersion => Ok(vec![make_single_text_response(
            "version",
            "PostgreSQL 15.0 on x86_64-pc-linux-gnu",
        )]),
        StatementKind::SelectVersionWithRdsCheck => {
            Ok(vec![make_version_with_rds_check_response()])
        }
        StatementKind::SelectOne => Ok(vec![make_single_int_response("?column?", 1)]),
        StatementKind::SelectCurrentSchema => {
            Ok(vec![make_single_text_response("current_schema", "public")])
        }
        StatementKind::SelectCurrentDatabase => Ok(vec![make_single_text_response(
            "current_database",
            "ducklake",
        )]),
        StatementKind::SelectPgType => Ok(vec![make_pg_type_response()]),
        StatementKind::ShowVariable(ref var) => {
            let val = get_show_value(var, session);
            Ok(vec![make_single_text_response(var, &val)])
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
            Ok(vec![make_null_text_response("to_regclass")])
        }
        StatementKind::SelectExistsInfoSchema => {
            // EXISTS(SELECT 1 FROM information_schema.tables WHERE ...) — return
            // false; RockLake does not expose information_schema.
            Ok(vec![make_false_bool_response("exists")])
        }
        StatementKind::SelectPgDatabaseSize => {
            // pg_database_size(current_database()) — informational only; return 0.
            Ok(vec![make_single_int_response("pg_database_size", 0)])
        }
        StatementKind::PgCatalogScan => {
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
                        )]);
                    }
                }
            }
            // Multi-statement pg_namespace / pg_class / pg_enum / pg_type /
            // pg_indexes catalog scan sent by the DuckDB postgres scanner as a
            // single PQsendQuery call. Return five result sets + ROLLBACK.
            Ok(make_pg_catalog_scan_responses())
        }

        // ─── Transaction Control ───────────────────────────────────────
        StatementKind::Begin => {
            session.in_transaction = true;
            Ok(vec![Response::TransactionStart(Tag::new("BEGIN"))])
        }
        StatementKind::Commit => {
            let bootstrap = std::mem::take(&mut session.bootstrap);
            let ops = session.pending_txn.take();
            session.in_transaction = false;

            // If this transaction contained COPY FROM STDIN bootstrap data
            // (i.e. DuckDB ATTACH initialisation), convert those rows into
            // catalog ops — but only if the catalog is still fresh (no prior
            // snapshot).  Subsequent ATTACH on an already-populated catalog
            // must NOT re-create the initial schema/snapshot.
            let mut all_ops: Vec<BufferedOp> = Vec::new();
            if bootstrap.has_snapshot || !bootstrap.schemas.is_empty() {
                let reader = { store.lock().await.read_latest() };
                let existing = reader.get_snapshot().await.map_err(RockLakeError::from)?;
                if existing.is_none() {
                    // Fresh catalog: create schemas first so next_catalog_id
                    // is correctly captured in the subsequent snapshot row.
                    for schema_row in bootstrap.schemas {
                        all_ops.push(BufferedOp::InsertSchema {
                            schema_name: schema_row.schema_name,
                        });
                    }
                    if bootstrap.has_snapshot {
                        all_ops.push(BufferedOp::InsertSnapshot {
                            author: None,
                            message: Some("DuckDB ATTACH bootstrap".to_string()),
                        });
                    }
                } else {
                    // Catalog already initialised; skip bootstrap ops.
                }
            }
            all_ops.extend(ops);
            execute_commit(all_ops, store, notify_manager).await?;
            Ok(vec![Response::TransactionEnd(Tag::new("COMMIT"))])
        }
        StatementKind::Rollback => {
            session.pending_txn.clear();
            session.in_transaction = false;
            Ok(vec![Response::TransactionEnd(Tag::new("ROLLBACK"))])
        }

        // ─── Read Operations ───────────────────────────────────────────
        StatementKind::SelectMaxSnapshot => {
            // F-11: clone reader out of mutex, drop lock before async I/O.
            let reader = { store.lock().await.read_latest() };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            let id = snap.map(|s| s.snapshot_id).unwrap_or(0);
            Ok(vec![make_single_int_response("max", id as i64)])
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
                    s.snapshot_id as i64,
                )]),
                None => Ok(vec![make_null_int_response("ducklake_latest_snapshot_id")]),
            }
        }
        StatementKind::SelectLatestSnapshotInfo => {
            let reader = { store.lock().await.read_latest() };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            Ok(vec![make_latest_snapshot_info_response(snap)])
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
            let snap_id = get_snapshot_param(params);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
            Ok(vec![make_schemas_response(schemas)])
        }
        StatementKind::SelectTables => {
            let schema_id = resolve_comparison_u64(_sql, "schema_id", params);
            let snap_id =
                resolve_comparison_u64(_sql, "begin_snapshot", params).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
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
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
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
            Ok(vec![make_columns_response(columns)])
        }
        StatementKind::SelectDataFiles => {
            let table_id = resolve_comparison_u64(_sql, "table_id", params);
            let snap_id =
                resolve_comparison_u64(_sql, "begin_snapshot", params).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let files = if let Some(table_id) = table_id {
                reader
                    .list_data_files(table_id)
                    .await
                    .map_err(RockLakeError::from)?
            } else {
                let schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
                let mut files = Vec::new();
                for schema in schemas {
                    for table in reader
                        .list_tables(schema.schema_id)
                        .await
                        .map_err(RockLakeError::from)?
                    {
                        files.extend(
                            reader
                                .list_data_files(table.table_id)
                                .await
                                .map_err(RockLakeError::from)?,
                        );
                    }
                }
                files
            };
            Ok(vec![make_data_files_response(files)])
        }
        StatementKind::SelectFileColumnStats => {
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
                )]);
            };
            let snap_id = params.get_u64(2).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let predicate = params.get(3).unwrap_or("");
            if predicate.is_empty() {
                let rows = reader
                    .list_file_column_stats(table_id, column_id)
                    .await
                    .map_err(RockLakeError::from)?;
                return Ok(vec![make_file_column_stats_response(_sql, rows)]);
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
            let table_id = params.get_u64(0).ok();
            let snap_id = params.get_u64(1).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            if _sql
                .to_ascii_lowercase()
                .contains("ducklake_table_column_stats")
            {
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
            let reader = { store.lock().await.read_latest() };
            let rows = reader
                .list_all_table_column_stats()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_table_column_stats_response(rows)])
        }
        StatementKind::SelectDeleteFiles => {
            let table_id = params.get_u64(0).ok();
            let snap_id = params.get_u64(1).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let files = if let Some(table_id) = table_id {
                reader
                    .list_delete_files(table_id)
                    .await
                    .map_err(RockLakeError::from)?
            } else {
                let schemas = reader.list_schemas().await.map_err(RockLakeError::from)?;
                let mut files = Vec::new();
                for schema in schemas {
                    for table in reader
                        .list_tables(schema.schema_id)
                        .await
                        .map_err(RockLakeError::from)?
                    {
                        files.extend(
                            reader
                                .list_delete_files(table.table_id)
                                .await
                                .map_err(RockLakeError::from)?,
                        );
                    }
                }
                files
            };
            Ok(vec![make_delete_files_response(files)])
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
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let rows = reader
                .list_all_snapshot_changes()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_snapshot_changes_response(rows)])
        }
        StatementKind::SelectMetadata => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let rows = reader
                .list_all_metadata()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_metadata_response(rows)])
        }
        StatementKind::SelectViews => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let rows = reader.list_all_views().await.map_err(RockLakeError::from)?;
            Ok(vec![make_views_response(rows)])
        }
        StatementKind::SelectMacros => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let rows = reader
                .list_all_macros()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_macros_response(rows)])
        }
        StatementKind::SelectMacroImpls => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
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
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
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
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let rows = reader.list_all_tags().await.map_err(RockLakeError::from)?;
            Ok(vec![make_tags_response(rows)])
        }
        StatementKind::SelectColumnTags => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let rows = reader
                .list_all_column_tags()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_column_tags_response(rows)])
        }
        StatementKind::SelectSortInfo => {
            let snap_id = params.get_u64(0).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
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
            let reader = { store.lock().await.read_latest() };
            let rows = reader
                .list_all_schema_versions()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_schema_versions_response(rows)])
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
            let table_id = params.get_u64(0).ok();
            let reader = { store.lock().await.read_latest() };
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
            Ok(vec![make_partition_info_response(rows)])
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
            Ok(vec![make_partition_columns_response(rows)])
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
            let rows = reader
                .list_files_scheduled_for_deletion()
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_files_scheduled_for_deletion_response(rows)])
        }

        StatementKind::SelectDuckLakeMetadataTable { ref table_name } => {
            Ok(vec![make_metadata_table_empty_response(table_name)])
        }

        StatementKind::SelectInlinedData => {
            let table_id = resolve_comparison_u64(_sql, "table_id", params);
            let reader = { store.lock().await.read_latest() };
            let rows = reader
                .list_inlined_data_tables(table_id)
                .await
                .map_err(RockLakeError::from)?;
            Ok(vec![make_inlined_data_tables_response(rows)])
        }
        StatementKind::SelectInlinedRows => {
            let Some(table_name) = inlined_table_name_from_sql(_sql) else {
                return Ok(vec![make_empty_response()]);
            };
            let Some((table_id, schema_version)) = parse_inlined_table_ids(&table_name) else {
                return Ok(vec![make_empty_response()]);
            };
            let reader = { store.lock().await.read_latest() };
            let Some((_, columns)) = reader
                .describe_table(table_id)
                .await
                .map_err(RockLakeError::from)?
            else {
                return Ok(vec![make_empty_response()]);
            };
            let raw_inserts = reader
                .list_inlined_inserts(table_id)
                .await
                .map_err(RockLakeError::from)?;
            let rows = raw_inserts
                .into_iter()
                .filter(|row| row.schema_version == schema_version)
                .collect::<Vec<_>>();
            Ok(vec![make_inlined_rows_response(_sql, columns, rows)])
        }

        // ─── pg-tide-relay extensions ──────────────────────────────────
        StatementKind::SelectMaxSnapshotAfter => {
            let after_id = params.get_u64(0).unwrap_or(0);
            let reader = { store.lock().await.read_latest() };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            let id = snap.map(|s| s.snapshot_id).unwrap_or(0);
            if id > after_id {
                Ok(vec![make_single_int_response("max", id as i64)])
            } else {
                Ok(vec![make_null_int_response("max")])
            }
        }
        StatementKind::SelectFirstSnapshot => {
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(1))
                    .map_err(RockLakeError::from)?
            };
            let snap = reader.get_snapshot().await.map_err(RockLakeError::from)?;
            if let Some(snap) = snap {
                Ok(vec![make_snapshot_row_response(snap)])
            } else {
                Ok(vec![make_empty_response()])
            }
        }
        StatementKind::SelectDataFilesWithLimit => {
            let table_id = require_param_u64(params, 0, "table_id")?;
            let limit = params.get_u64(1).unwrap_or(u64::MAX);
            let snap_id = params.get_u64(2).unwrap_or(u64::MAX);
            let reader = {
                let s = store.lock().await;
                s.read_at(rocklake_core::mvcc::SnapshotId::new(snap_id))
                    .map_err(RockLakeError::from)?
            };
            let mut files = reader
                .list_data_files(table_id)
                .await
                .map_err(RockLakeError::from)?;
            files.truncate(limit as usize);
            Ok(vec![make_data_files_response(files)])
        }
        StatementKind::SelectGenRandomUuid => {
            let uuid_val = uuid::Uuid::new_v4().to_string();
            Ok(vec![make_single_text_response(
                "gen_random_uuid",
                &uuid_val,
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
                change_type: params
                    .get_string(0)
                    .ok()
                    .or_else(|| literal_string(&literals, 1))
                    .unwrap_or_default(),
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
            let mut schema_name = String::new();
            if let Some(rows) = parse_insert_rows_map(_sql, params) {
                if let Some(row) = rows.first() {
                    schema_name = row
                        .get("schema_name")
                        .and_then(|v| v.clone())
                        .unwrap_or_default();
                }
            } else {
                let literals = literal_insert_values(_sql);
                schema_name = params
                    .get_string(0)
                    .ok()
                    .or_else(|| literal_string(&literals, 4))
                    .unwrap_or_default();
            }
            let op = BufferedOp::InsertSchema { schema_name };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
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
                        .unwrap_or(1);
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
                schema_id = params
                    .get_u64(0)
                    .ok()
                    .or_else(|| literal_u64(&literals, 4))
                    .unwrap_or(1);
                table_name = params
                    .get_string(1)
                    .ok()
                    .or_else(|| literal_string(&literals, 5))
                    .unwrap_or_default();
                data_path = params
                    .get_optional_string(2)
                    .or_else(|| literal_string(&literals, 6));
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
                    let table_id = row
                        .get("table_id")
                        .and_then(|v| v.as_ref().and_then(|s| s.parse::<u64>().ok()))
                        .unwrap_or(0);
                    let column_name = row
                        .get("column_name")
                        .and_then(|v| v.clone())
                        .unwrap_or_default();
                    let data_type = row
                        .get("column_type")
                        .and_then(|v| v.clone())
                        .or_else(|| row.get("data_type").and_then(|v| v.clone()))
                        .unwrap_or_default();
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
                        table_id: params
                            .get_u64(0)
                            .ok()
                            .or_else(|| literal_u64(&literals, 3))
                            .unwrap_or(0),
                        column_name: params
                            .get_string(1)
                            .ok()
                            .or_else(|| literal_string(&literals, 5))
                            .unwrap_or_default(),
                        data_type: params
                            .get_string(2)
                            .ok()
                            .or_else(|| literal_string(&literals, 6))
                            .unwrap_or_default(),
                        column_index: params
                            .get_u64(3)
                            .ok()
                            .or_else(|| literal_u64(&literals, 4))
                            .unwrap_or(0),
                        is_nullable: params
                            .get_bool(4)
                            .ok()
                            .or_else(|| literal_bool(&literals, 9))
                            .unwrap_or(true),
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
                    .or_else(|| literal_string(&literals, 5))
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
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertDeleteFile => {
            let op = BufferedOp::InsertDeleteFile {
                data_file_id: params.get_u64(0).unwrap_or(0),
                path: params.get_string(1).unwrap_or_default(),
                delete_count: params.get_u64(2).unwrap_or(0),
                file_size_bytes: params.get_u64(3).unwrap_or(0),
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
            let op = BufferedOp::InsertTableStats {
                table_id: params
                    .get_u64(0)
                    .ok()
                    .or_else(|| literal_u64(&literals, 0))
                    .unwrap_or(0),
                record_count: params
                    .get_u64(1)
                    .ok()
                    .or_else(|| literal_u64(&literals, 1))
                    .unwrap_or(0),
                // DuckLake v1.0 position 2 is next_row_id, not file_count.
                next_row_id: params
                    .get_u64(2)
                    .ok()
                    .or_else(|| literal_u64(&literals, 2))
                    .unwrap_or(0),
                file_size_bytes: params
                    .get_u64(3)
                    .ok()
                    .or_else(|| literal_u64(&literals, 3))
                    .unwrap_or(0),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertFileColumnStats => {
            let op = BufferedOp::InsertFileColumnStats {
                table_id: params.get_u64(0).unwrap_or(0),
                column_id: params.get_u64(1).unwrap_or(0),
                data_file_id: params.get_u64(2).unwrap_or(0),
                contains_null: params.get_bool(3).unwrap_or(false),
                min_value: params.get_optional_string(4),
                max_value: params.get_optional_string(5),
                contains_nan: params.get_bool(6).unwrap_or(false),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertTableColumnStats => {
            let literal_rows = literal_insert_rows(_sql);
            let parameterized = params.get_u64(0).is_ok();
            let mut ops = Vec::new();
            if parameterized || literal_rows.is_empty() {
                let literals = literal_rows.into_iter().next().unwrap_or_default();
                ops.push(BufferedOp::InsertTableColumnStats {
                    table_id: params
                        .get_u64(0)
                        .ok()
                        .or_else(|| literal_u64(&literals, 0))
                        .unwrap_or(0),
                    column_id: params
                        .get_u64(1)
                        .ok()
                        .or_else(|| literal_u64(&literals, 1))
                        .unwrap_or(0),
                    contains_null: params
                        .get_bool(2)
                        .ok()
                        .or_else(|| literal_bool(&literals, 2))
                        .unwrap_or(false),
                    contains_nan: params
                        .get_bool(3)
                        .ok()
                        .or_else(|| literal_bool(&literals, 3)),
                    min_value: params
                        .get_optional_string(4)
                        .or_else(|| literal_string(&literals, 4)),
                    max_value: params
                        .get_optional_string(5)
                        .or_else(|| literal_string(&literals, 5)),
                    extra_stats: params
                        .get_optional_string(6)
                        .or_else(|| literal_string(&literals, 6)),
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
            let ops: Vec<BufferedOp> = literal_insert_rows(_sql)
                .into_iter()
                .map(|literals| BufferedOp::InsertTableColumnStats {
                    table_id: literal_u64(&literals, 0).unwrap_or(0),
                    column_id: literal_u64(&literals, 1).unwrap_or(0),
                    contains_null: literal_bool(&literals, 2).unwrap_or(false),
                    contains_nan: literal_bool(&literals, 3),
                    min_value: literal_string(&literals, 4),
                    max_value: literal_string(&literals, 5),
                    extra_stats: literal_string(&literals, 6),
                })
                .collect();
            let row_count = ops.len();
            if session.in_transaction {
                for op in ops {
                    session.pending_txn.push(op)?;
                }
            } else {
                execute_commit(ops, store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new(&format!(
                "UPDATE {row_count}"
            )))])
        }
        StatementKind::InsertMetadata => {
            let op = BufferedOp::InsertMetadata {
                key: params.get_string(0).unwrap_or_default(),
                value: params.get_string(1).unwrap_or_default(),
                scope: params.get_optional_string(2),
                scope_id: params.get_u64(3).ok(),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertSchemaVersions => {
            let literal_rows = literal_insert_rows(_sql);
            let parameterized = params.get_u64(0).is_ok();
            let mut ops = Vec::new();
            if parameterized || literal_rows.is_empty() {
                let literals = literal_rows.into_iter().next().unwrap_or_default();
                ops.push(BufferedOp::InsertSchemaVersions {
                    begin_snapshot: params
                        .get_u64(0)
                        .ok()
                        .or_else(|| literal_u64(&literals, 0))
                        .unwrap_or(0),
                    schema_version: params
                        .get_u64(1)
                        .ok()
                        .or_else(|| literal_u64(&literals, 1))
                        .unwrap_or(0),
                    table_id: params
                        .get_u64(2)
                        .ok()
                        .or_else(|| literal_u64(&literals, 2))
                        .unwrap_or(0),
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
            let parameterized = params.get_u64(0).is_ok();
            let mut ops = Vec::new();
            if parameterized || literal_rows.is_empty() {
                let literals = literal_rows.into_iter().next().unwrap_or_default();
                ops.push(BufferedOp::InsertInlinedDataTables {
                    table_id: params
                        .get_u64(0)
                        .ok()
                        .or_else(|| literal_u64(&literals, 0))
                        .unwrap_or(0),
                    table_name: params
                        .get_string(1)
                        .ok()
                        .or_else(|| literal_string(&literals, 1))
                        .unwrap_or_default(),
                    schema_version: params
                        .get_u64(2)
                        .ok()
                        .or_else(|| literal_u64(&literals, 2))
                        .unwrap_or(0),
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
            let op = BufferedOp::InsertView {
                schema_id: params.get_u64(0).unwrap_or(0),
                view_name: params.get_string(1).unwrap_or_default(),
                sql: params.get_string(2).unwrap_or_default(),
                view_uuid: params.get_optional_string(3),
                dialect: params.get_optional_string(4),
                column_aliases: params.get_optional_string(5),
            };
            if session.in_transaction {
                session.pending_txn.push(op)?;
            } else {
                execute_commit(vec![op], store, notify_manager).await?;
            }
            Ok(vec![Response::Execution(Tag::new("INSERT 0 1"))])
        }
        StatementKind::InsertMacro => {
            let op = BufferedOp::InsertMacro {
                schema_id: params.get_u64(0).unwrap_or(0),
                macro_name: params.get_string(1).unwrap_or_default(),
                macro_type: params.get_string(2).unwrap_or_default(),
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
            let op = BufferedOp::InsertMacroImpl {
                macro_id: params.get_u64(0).unwrap_or(0),
                sql: params.get_string(1).unwrap_or_default(),
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
            let op = BufferedOp::InsertMacroParams {
                macro_id: params.get_u64(0).unwrap_or(0),
                impl_id: params.get_u64(1).unwrap_or(0),
                column_id: params.get_u64(2).unwrap_or(0),
                parameter_name: params.get_string(3).unwrap_or_default(),
                parameter_type: params.get_string(4).unwrap_or_default(),
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
            let mut entity_id = params.get_u64(1).unwrap_or(0);
            let begin_snapshot = params.get_u64(2).unwrap_or(0);
            let mut end_snapshot = params.get_u64(0).unwrap_or(0);
            let mut where_column = None;

            if let Some(extracted) = extract_id_from_where_clause(_sql) {
                if entity_id == 0 {
                    entity_id = extracted.value;
                }
                where_column = Some(extracted.column);
            }

            if end_snapshot == 0 {
                if let Some(val) = literal_assignment_u64(_sql, "end_snapshot") {
                    end_snapshot = val;
                }
            }

            let op = BufferedOp::UpdateEndSnapshot {
                table_name: table_name.clone(),
                entity_id,
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
            } else {
                BufferedOp::UpdateTableStats {
                    table_id: params.get_u64(1).unwrap_or(0),
                    row_count_delta: params.get_i64(0).unwrap_or(0),
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
            // Accept CREATE TABLE for inlined tables (no-op, tracked in catalog)
            Ok(vec![Response::Execution(Tag::new("CREATE TABLE"))])
        }
        StatementKind::InsertInlinedRow => {
            let table_name = ducklake_insert_table_name(_sql).unwrap_or_default();
            if table_name == "ducklake_column_mapping" {
                let row_count = execute_ducklake_column_mapping_insert(_sql, params, store).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            if table_name == "ducklake_name_mapping" {
                let row_count = execute_ducklake_name_mapping_insert(_sql, params, store).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            if table_name == "ducklake_partition_info" {
                let row_count = execute_ducklake_partition_info_insert(_sql, params, store).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            if table_name == "ducklake_sort_info" {
                let row_count = execute_ducklake_sort_info_insert(_sql, params, store).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            if table_name == "ducklake_files_scheduled_for_deletion" {
                let row_count =
                    execute_ducklake_files_scheduled_for_deletion_insert(_sql, params, store)
                        .await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            if table_name == "ducklake_partition_column" {
                let row_count =
                    execute_ducklake_partition_column_insert(_sql, params, store).await?;
                return Ok(vec![Response::Execution(Tag::new(&format!(
                    "INSERT 0 {row_count}"
                )))]);
            }
            if table_name == "ducklake_sort_expression" {
                let row_count =
                    execute_ducklake_sort_expression_insert(_sql, params, store).await?;
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
            execute_virtual_catalog_scan(table_name, store).await
        }

        // DELETE FROM "public".ducklake_inlined_data_<tid>_<sv> WHERE ctid IN (...)
        // Issued by DuckLake CHECKPOINT after flushing inlined rows to Parquet.
        StatementKind::DeleteInlinedDataRows { ref table_name } => {
            let mut row_ids = row_ids_from_ctid_sql(_sql);
            if row_ids.is_empty() {
                if let Some(snapshot_threshold) =
                    resolve_comparison_u64(_sql, "begin_snapshot", params)
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
            // Record which table is being loaded so on_copy_data / on_copy_done
            // can accumulate and parse the binary stream correctly.
            session.pending_copy = Some(CopyAccumulator {
                table: table.clone(),
                data: Vec::new(),
            });
            // Return CopyIn response with binary format (1).
            // format=1 (binary), columns=0 (unspecified), empty column_formats.
            Ok(vec![Response::CopyIn(CopyResponse::new(1, 0, vec![]))])
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
