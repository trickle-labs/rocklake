//! Shared response-builder helpers and parameter utilities for the executor.

use std::sync::Arc;

use pgwire::api::results::{DataRowEncoder, FieldFormat, FieldInfo, QueryResponse, Response, Tag};
use pgwire::api::Type;

use rocklake_core::rows::ColumnRow;
use rocklake_sql::ParamValues;

use crate::error::RockLakeError;
use crate::session::SessionState;
use crate::types;

/// F-24: Require a u64 parameter; returns SQLSTATE 22023 if absent or invalid.
pub(super) fn require_param_u64(
    params: &ParamValues,
    idx: usize,
    name: &str,
) -> Result<u64, RockLakeError> {
    params
        .get_u64(idx)
        .map_err(|_| RockLakeError::MissingParam {
            name: name.to_string(),
        })
}

pub(super) fn get_snapshot_param(params: &ParamValues) -> u64 {
    params.get_u64(0).unwrap_or(u64::MAX)
}

pub(super) async fn resolve_reader(
    store: &Arc<tokio::sync::Mutex<rocklake_catalog::CatalogStore>>,
    session: &SessionState,
    snap_id: u64,
) -> Result<rocklake_catalog::reader::CatalogReader, RockLakeError> {
    let s = store.lock().await;
    if snap_id >= (i64::MAX as u64) {
        if session.in_transaction {
            if let Some(tx_snap) = session.transaction_snapshot_id {
                return s
                    .read_at(rocklake_core::mvcc::SnapshotId::new(tx_snap))
                    .map_err(RockLakeError::from);
            }
        }
        Ok(s.read_latest())
    } else if session.in_transaction {
        if let Some(tx_snap) = session.transaction_snapshot_id {
            let effective_snap = snap_id.min(tx_snap);
            s.read_at(rocklake_core::mvcc::SnapshotId::new(effective_snap))
                .map_err(RockLakeError::from)
        } else {
            let effective_snap = snap_id.min(s.latest_committed_snapshot_id());
            s.read_at(rocklake_core::mvcc::SnapshotId::new(effective_snap))
                .map_err(RockLakeError::from)
        }
    } else {
        let effective_snap = snap_id.min(s.latest_committed_snapshot_id());
        s.read_at(rocklake_core::mvcc::SnapshotId::new(effective_snap))
            .map_err(RockLakeError::from)
    }
}

pub(super) fn get_show_value(var: &str, session: &SessionState) -> String {
    match var.to_lowercase().as_str() {
        "server_version" => "15.0".to_string(),
        "datestyle" | "date_style" => session.settings.date_style.clone(),
        "timezone" | "time zone" => session.settings.timezone.clone(),
        "client_encoding" => session.settings.client_encoding.clone(),
        "transaction_isolation" => "read committed".to_string(),
        "standard_conforming_strings" => "on".to_string(),
        // Fall back to the generic extra map for any driver-specific variable.
        other => session
            .settings
            .extra
            .get(other)
            .cloned()
            .unwrap_or_default(),
    }
}

pub(super) fn apply_set(var: &str, val: &str, session: &mut SessionState) {
    let clean_val = val.trim_matches('\'').to_string();
    match var.to_lowercase().as_str() {
        "timezone" | "time zone" => session.settings.timezone = clean_val,
        "client_encoding" => session.settings.client_encoding = clean_val,
        "datestyle" | "date_style" => session.settings.date_style = clean_val,
        "application_name" => session.settings.application_name = clean_val,
        // Store any unknown setting in the generic extra map so that driver-
        // specific SET commands (e.g. SET synchronize_seqscans = off) are
        // accepted and retrievable via SHOW without crashing or returning errors.
        other => {
            session.settings.extra.insert(other.to_string(), clean_val);
        }
    }
}

pub(super) fn make_single_text_response<'a>(col_name: &str, value: Option<&str>) -> Response<'a> {
    let schema = Arc::new(vec![FieldInfo::new(
        col_name.to_string(),
        None,
        None,
        Type::TEXT,
        FieldFormat::Text,
    )]);
    let mut encoder = DataRowEncoder::new(schema.clone());
    encoder
        .encode_field_with_type_and_format(
            &value.map(|value| value.to_string()),
            &Type::TEXT,
            FieldFormat::Text,
        )
        .unwrap();
    let row = encoder.finish();
    let data_rows = vec![row];
    let mut resp = QueryResponse::new(schema, futures::stream::iter(data_rows));
    resp.set_command_tag("SELECT 1");
    Response::Query(resp)
}

pub(super) fn make_single_int_response<'a>(col_name: &str, value: Option<i64>) -> Response<'a> {
    let schema = Arc::new(vec![FieldInfo::new(
        col_name.to_string(),
        None,
        None,
        Type::INT8,
        FieldFormat::Binary,
    )]);
    let mut encoder = DataRowEncoder::new(schema.clone());
    encoder
        .encode_field_with_type_and_format(&value, &Type::INT8, FieldFormat::Binary)
        .unwrap();
    let row = encoder.finish();
    let data_rows = vec![row];
    let mut resp = QueryResponse::new(schema, futures::stream::iter(data_rows));
    resp.set_command_tag("SELECT 1");
    Response::Query(resp)
}

pub(super) fn make_pg_type_response<'a>() -> Response<'a> {
    let schema = Arc::new(vec![
        FieldInfo::new("oid".to_string(), None, None, Type::INT4, FieldFormat::Text),
        FieldInfo::new(
            "typname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
    ]);
    let mut data_rows = Vec::new();
    for (name, oid) in types::PG_TYPE_MAP {
        let mut encoder = DataRowEncoder::new(schema.clone());
        encoder
            .encode_field_with_type_and_format(
                &Some(oid.to_string()),
                &Type::TEXT,
                FieldFormat::Text,
            )
            .unwrap();
        encoder
            .encode_field_with_type_and_format(
                &Some(name.to_string()),
                &Type::TEXT,
                FieldFormat::Text,
            )
            .unwrap();
        data_rows.push(encoder.finish());
    }
    let count = data_rows.len();
    let mut resp = QueryResponse::new(schema, futures::stream::iter(data_rows));
    resp.set_command_tag(&format!("SELECT {count}"));
    Response::Query(resp)
}

pub(super) fn make_empty_response<'a>() -> Response<'a> {
    let schema = Arc::new(vec![]);
    let resp = QueryResponse::new(schema, futures::stream::iter(Vec::new()));
    Response::Query(resp)
}

pub(super) fn make_version_with_rds_check_response<'a>() -> Response<'a> {
    let schema = Arc::new(vec![
        FieldInfo::new(
            "version".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "count".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
    ]);
    let mut encoder = DataRowEncoder::new(schema.clone());
    encoder
        .encode_field_with_type_and_format(
            &Some("PostgreSQL 15.0 on x86_64-pc-linux-gnu".to_string()),
            &Type::TEXT,
            FieldFormat::Text,
        )
        .expect("pgwire field encoding is infallible");
    // RDS check: 0 means not on RDS
    encoder
        .encode_field_with_type_and_format(&Some("0".to_string()), &Type::INT8, FieldFormat::Text)
        .expect("pgwire field encoding is infallible");
    let row = encoder.finish();
    let data_rows = vec![row];
    let mut resp = QueryResponse::new(schema, futures::stream::iter(data_rows));
    resp.set_command_tag("SELECT 1");
    Response::Query(resp)
}

/// `SELECT EXISTS(...)` — returns a single `false` BOOL row.
pub(super) fn make_false_bool_response<'a>(col_name: &str) -> Response<'a> {
    let schema = Arc::new(vec![FieldInfo::new(
        col_name.to_string(),
        None,
        None,
        Type::BOOL,
        FieldFormat::Text,
    )]);
    let mut encoder = DataRowEncoder::new(schema.clone());
    // PostgreSQL text format for boolean false is "f"
    encoder
        .encode_field_with_type_and_format(&Some("f".to_string()), &Type::TEXT, FieldFormat::Text)
        .expect("pgwire field encoding is infallible");
    let row = encoder.finish();
    let data_rows = vec![row];
    let mut resp = QueryResponse::new(schema, futures::stream::iter(data_rows));
    resp.set_command_tag("SELECT 1");
    Response::Query(resp)
}

/// Build one result-set row for the `pg_namespace` response.
/// Columns: `(oid INT8, nspname TEXT)`
fn make_pg_namespace_schema() -> Arc<Vec<FieldInfo>> {
    Arc::new(vec![
        FieldInfo::new("oid".to_string(), None, None, Type::INT8, FieldFormat::Text),
        FieldInfo::new(
            "nspname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
    ])
}

fn make_pg_namespace_rows(
    schema: Arc<Vec<FieldInfo>>,
    namespaces: &[(i64, &str)],
) -> Vec<pgwire::error::PgWireResult<pgwire::messages::data::DataRow>> {
    namespaces
        .iter()
        .map(|(oid, name)| {
            let mut enc = DataRowEncoder::new(schema.clone());
            enc.encode_field_with_type_and_format(
                &Some(oid.to_string()),
                &Type::TEXT,
                FieldFormat::Text,
            )
            .expect("infallible");
            enc.encode_field_with_type_and_format(
                &Some(name.to_string()),
                &Type::TEXT,
                FieldFormat::Text,
            )
            .expect("infallible");
            enc.finish()
        })
        .collect()
}

/// Build the `pg_class` UNION result-set schema.
/// Columns: `(namespace_id INT8, relname TEXT, relpages INT8, attname TEXT,
///  type_name TEXT, type_modifier INT8, ndim INT8, attnum INT8,
///  notnull BOOL, constraint_id INT8, constraint_type TEXT, constraint_key TEXT)`
// DuckDB 2.0's PostgreSQL scanner projects type_schema and column comments
// in addition to the 12 fields requested by DuckDB 1.5.x. Select the wire
// response shape from the query, preserving older client compatibility.
fn pg_class_column_count(sql: &str) -> usize {
    let lower = sql.to_ascii_lowercase();
    if lower.contains("type_schema") {
        if lower.contains("column_default") {
            16
        } else {
            15
        }
    } else {
        12
    }
}

fn make_pg_class_schema(sql: &str) -> Arc<Vec<FieldInfo>> {
    let mut fields = vec![
        FieldInfo::new(
            "namespace_id".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "relname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "relpages".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "attname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "type_name".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "type_modifier".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "ndim".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "attnum".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "notnull".to_string(),
            None,
            None,
            Type::BOOL,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "constraint_id".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "constraint_type".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "constraint_key".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
    ];
    if pg_class_column_count(sql) >= 15 {
        for name in ["type_schema", "column_comment", "table_comment"] {
            fields.push(FieldInfo::new(
                name.to_string(),
                None,
                None,
                Type::TEXT,
                FieldFormat::Text,
            ));
        }
    }
    if pg_class_column_count(sql) >= 16 {
        fields.push(FieldInfo::new(
            "column_default".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ));
    }
    Arc::new(fields)
}

pub(super) fn make_pg_catalog_inlined_table_response<'a>(
    table_name: &str,
    columns: Vec<ColumnRow>,
    sql: &str,
) -> Response<'a> {
    let schema = make_pg_class_schema(sql);
    let mut rows = Vec::new();
    let mut attnum = 1i64;

    for (name, type_name) in [
        ("row_id".to_string(), "int8".to_string()),
        ("begin_snapshot".to_string(), "int8".to_string()),
        ("end_snapshot".to_string(), "int8".to_string()),
    ] {
        rows.push(make_pg_class_attribute_row(
            schema.clone(),
            table_name,
            &name,
            &type_name,
            attnum,
        ));
        attnum += 1;
    }

    for column in columns {
        rows.push(make_pg_class_attribute_row(
            schema.clone(),
            table_name,
            &column.column_name,
            &pg_type_name_for_inlined_column(&column.data_type),
            attnum,
        ));
        attnum += 1;
    }

    let count = rows.len();
    let mut resp = QueryResponse::new(schema, futures::stream::iter(rows));
    resp.set_command_tag(&format!("SELECT {count}"));
    Response::Query(resp)
}

fn make_pg_class_attribute_row(
    schema: Arc<Vec<FieldInfo>>,
    table_name: &str,
    attname: &str,
    type_name: &str,
    attnum: i64,
) -> pgwire::error::PgWireResult<pgwire::messages::data::DataRow> {
    let column_count = schema.len();
    let mut enc = DataRowEncoder::new(schema);
    let mut values = vec![
        Some("1".to_string()),
        Some(table_name.to_string()),
        Some("0".to_string()),
        Some(attname.to_string()),
        Some(type_name.to_string()),
        Some("-1".to_string()),
        Some("0".to_string()),
        Some(attnum.to_string()),
        Some("false".to_string()),
        None,
        None,
        None,
    ];
    if column_count >= 15 {
        values.push(Some("pg_catalog".to_string()));
        values.push(None);
        values.push(None);
    }
    if column_count >= 16 {
        values.push(None);
    }
    for value in values {
        enc.encode_field_with_type_and_format(&value, &Type::TEXT, FieldFormat::Text)
            .expect("infallible");
    }
    enc.finish()
}

/// PostgreSQL type names for the DuckDB scanner's pg_class catalog probe.
///
/// The scanner builds relation schemas from these names, so the advertised
/// types must agree with the shared DuckLake metadata schema registry.
fn pg_type_name_for_catalog_field(field: &FieldInfo) -> &'static str {
    let datatype = field.datatype();
    if datatype == &Type::INT8 {
        "int8"
    } else if datatype == &Type::INT4 {
        "int4"
    } else if datatype == &Type::INT2 {
        "int2"
    } else if datatype == &Type::BOOL {
        "bool"
    } else if datatype == &Type::UUID {
        "uuid"
    } else if datatype == &Type::TIMESTAMPTZ {
        "timestamptz"
    } else if datatype == &Type::TIMESTAMP {
        "timestamp"
    } else if datatype == &Type::DATE {
        "date"
    } else if datatype == &Type::BYTEA {
        "bytea"
    } else if datatype == &Type::FLOAT8 {
        "float8"
    } else if datatype == &Type::FLOAT4 {
        "float4"
    } else {
        "text"
    }
}

fn pg_type_name_for_inlined_column(logical_type: &str) -> String {
    match logical_type.to_ascii_lowercase().as_str() {
        "boolean" | "bool" => "bool".to_string(),
        "tinyint" | "int8" => "int2".to_string(),
        "smallint" | "int16" => "int2".to_string(),
        "integer" | "int" | "int4" | "int32" => "int4".to_string(),
        "bigint" | "int64" => "int8".to_string(),
        "varchar" | "text" | "string" | "blob" | "bytea" => "bytea".to_string(),
        "timestamp" | "timestamp without time zone" => "timestamp".to_string(),
        "timestamp with time zone" | "timestamptz" => "timestamptz".to_string(),
        "date" => "date".to_string(),
        _ => "text".to_string(),
    }
}

/// Build the `pg_enum` result-set schema.
/// Columns: `(oid INT8, enumtypid INT8, typname TEXT, enumlabel TEXT)`
fn make_pg_enum_schema() -> Arc<Vec<FieldInfo>> {
    Arc::new(vec![
        FieldInfo::new("oid".to_string(), None, None, Type::INT8, FieldFormat::Text),
        FieldInfo::new(
            "enumtypid".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "typname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "enumlabel".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
    ])
}

/// Build the `pg_type` composites result-set schema.
/// Columns: `(oid INT8, id INT8, type TEXT, attname TEXT, typname TEXT)`
fn make_pg_type_composites_schema() -> Arc<Vec<FieldInfo>> {
    Arc::new(vec![
        FieldInfo::new("oid".to_string(), None, None, Type::INT8, FieldFormat::Text),
        FieldInfo::new("id".to_string(), None, None, Type::INT8, FieldFormat::Text),
        FieldInfo::new(
            "type".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "attname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "typname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
    ])
}

/// Build the `pg_indexes` result-set schema.
/// Columns: `(oid INT8, tablename TEXT, indexname TEXT)`
fn make_pg_indexes_schema() -> Arc<Vec<FieldInfo>> {
    Arc::new(vec![
        FieldInfo::new("oid".to_string(), None, None, Type::INT8, FieldFormat::Text),
        FieldInfo::new(
            "tablename".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "indexname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
    ])
}

/// DuckDB's PostgreSQL catalog cache probes relation OIDs and xmin values to
/// detect metadata changes. This is a single query, not the five-result scan
/// batch, despite referencing both pg_class and pg_namespace.
pub(super) fn make_pg_catalog_staleness_response<'a>(
    snapshot_id: u64,
    initialized: bool,
) -> Response<'a> {
    let schema = Arc::new(vec![
        FieldInfo::new("oid".to_string(), None, None, Type::INT8, FieldFormat::Text),
        FieldInfo::new(
            "relname".to_string(),
            None,
            None,
            Type::TEXT,
            FieldFormat::Text,
        ),
        FieldInfo::new(
            "xmin".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        ),
    ]);
    let rows = if initialized {
        crate::schema_registry::supported_tables()
            .iter()
            .enumerate()
            .map(|(index, table)| {
                let mut enc = DataRowEncoder::new(schema.clone());
                for value in [
                    (index + 1).to_string(),
                    (*table).to_string(),
                    snapshot_id.to_string(),
                ] {
                    enc.encode_field_with_type_and_format(
                        &Some(value),
                        &Type::TEXT,
                        FieldFormat::Text,
                    )
                    .expect("infallible");
                }
                enc.finish()
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let count = rows.len();
    let mut response = QueryResponse::new(schema, futures::stream::iter(rows));
    response.set_command_tag(&format!("SELECT {count}"));
    Response::Query(response)
}

/// Return the five result sets for the DuckDB postgres-scanner catalog scan,
/// plus a ROLLBACK command-complete.
///
/// The batch is:
///   1. `pg_namespace` — `(1, 'public')` and `(2, 'main')`
///   2. `pg_class` UNION — column metadata for supported DuckLake v1.0 tables
///   3. `pg_enum` — empty
///   4. `pg_type` composites — empty
///   5. `pg_indexes` — empty
///   +  ROLLBACK command-complete tag
pub(super) fn make_pg_catalog_scan_responses<'a>(
    sql: &str,
    initialized: bool,
    bootstrap_relations: &std::collections::HashSet<String>,
) -> Vec<Response<'a>> {
    // Result set 1: pg_namespace
    let ns_schema = make_pg_namespace_schema();
    let ns_rows = make_pg_namespace_rows(ns_schema.clone(), &[(1, "public"), (2, "main")]);
    let mut ns_resp = QueryResponse::new(ns_schema, futures::stream::iter(ns_rows));
    ns_resp.set_command_tag("SELECT 2");

    // DuckDB 1.5.x expects an empty pg_class listing and initializes its
    // DuckLake catalog over the PG wire protocol itself. Advertising virtual
    // DuckLake relations there prevents initialization ("No snapshot found").
    // DuckDB 2.0's extended pg_class query needs the relations advertised.
    let cls_schema = make_pg_class_schema(sql);
    let mut cls_rows = Vec::new();
    // Do not advertise metadata tables before the first DuckLake snapshot:
    // DuckLake probes their existence to decide whether to bootstrap.
    if pg_class_column_count(sql) >= 15 {
        let lower = sql.to_ascii_lowercase();
        let requested_table = lower
            .split("relname=")
            .nth(1)
            .or_else(|| lower.split("relname =").nth(1))
            .and_then(|filter| filter.trim_start().strip_prefix('\''))
            .and_then(|filter| filter.split('\'').next());
        for table in crate::schema_registry::supported_tables() {
            if !initialized && !bootstrap_relations.contains(*table) {
                continue;
            }
            if requested_table.is_some_and(|requested| requested != *table) {
                continue;
            }
            let fields = crate::schema_registry::fields_for_table(table)
                .expect("supported DuckLake metadata table must have a schema");
            for (index, field) in fields.iter().enumerate() {
                cls_rows.push(make_pg_class_attribute_row(
                    cls_schema.clone(),
                    table,
                    field.name(),
                    pg_type_name_for_catalog_field(field),
                    (index + 1) as i64,
                ));
            }
        }
    }
    let count = cls_rows.len();
    let mut cls_resp = QueryResponse::new(cls_schema, futures::stream::iter(cls_rows));
    cls_resp.set_command_tag(&format!("SELECT {count}"));

    // Result set 3: pg_enum — empty
    let enum_schema = make_pg_enum_schema();
    let enum_rows: Vec<pgwire::error::PgWireResult<pgwire::messages::data::DataRow>> = Vec::new();
    let mut enum_resp = QueryResponse::new(enum_schema, futures::stream::iter(enum_rows));
    enum_resp.set_command_tag("SELECT 0");

    // Result set 4: pg_type composites — empty
    let typ_schema = make_pg_type_composites_schema();
    let typ_rows: Vec<pgwire::error::PgWireResult<pgwire::messages::data::DataRow>> = Vec::new();
    let mut typ_resp = QueryResponse::new(typ_schema, futures::stream::iter(typ_rows));
    typ_resp.set_command_tag("SELECT 0");

    // Result set 5: pg_indexes — empty
    let idx_schema = make_pg_indexes_schema();
    let idx_rows: Vec<pgwire::error::PgWireResult<pgwire::messages::data::DataRow>> = Vec::new();
    let mut idx_resp = QueryResponse::new(idx_schema, futures::stream::iter(idx_rows));
    idx_resp.set_command_tag("SELECT 0");

    vec![
        Response::Query(ns_resp),
        Response::Query(cls_resp),
        Response::Query(enum_resp),
        Response::Query(typ_resp),
        Response::Query(idx_resp),
        Response::TransactionEnd(Tag::new("ROLLBACK")),
    ]
}

#[cfg(test)]
mod pg_catalog_tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn initialized_duckdb_2_catalog_exposes_registered_columns() {
        let sql = "SELECT relname, type_schema FROM pg_class JOIN pg_namespace ON true";
        let response = make_pg_catalog_scan_responses(
            sql,
            true,
            &std::collections::HashSet::new(),
        )
            .into_iter()
            .nth(1)
            .expect("pg_class response");
        let Response::Query(query) = response else {
            panic!("expected pg_class Query response");
        };
        let stream = query.data_rows();
        futures::pin_mut!(stream);
        let mut count = 0;
        while let Some(row) = stream.next().await {
            row.expect("valid pg_class metadata row");
            count += 1;
        }
        let expected = crate::schema_registry::supported_tables()
            .iter()
            .map(|table| {
                crate::schema_registry::fields_for_table(table)
                    .expect("registered metadata schema")
                    .len()
            })
            .sum::<usize>();
        assert_eq!(count, expected);
    }
}
