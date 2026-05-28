//! Classifier tests for v0.33.0: Virtual catalog mutation detection.
//!
//! INSERT/UPDATE/DELETE against `rocklake_catalog.*` must classify as
//! `VirtualCatalogMutation` and map to SQLSTATE 25006 in the PG-wire executor.

use rocklake_sql::{classify_statement, StatementKind};

// ─── INSERT ───────────────────────────────────────────────────────────────────

#[test]
fn classify_insert_virtual_catalog_snapshot() {
    let sql = "INSERT INTO rocklake_catalog.ducklake_snapshot (snapshot_id) VALUES (42)";
    let kind = classify_statement(sql).unwrap();
    assert!(
        matches!(
            kind,
            StatementKind::VirtualCatalogMutation { ref table_name }
            if table_name == "ducklake_snapshot"
        ),
        "INSERT into rocklake_catalog.ducklake_snapshot must be VirtualCatalogMutation; got {kind:?}"
    );
}

#[test]
fn classify_insert_virtual_catalog_schema() {
    let sql =
        "INSERT INTO rocklake_catalog.ducklake_schema (schema_id, schema_name) VALUES (1, 'main')";
    let kind = classify_statement(sql).unwrap();
    assert!(
        matches!(kind, StatementKind::VirtualCatalogMutation { .. }),
        "INSERT into rocklake_catalog.* must be VirtualCatalogMutation; got {kind:?}"
    );
}

// ─── UPDATE ───────────────────────────────────────────────────────────────────

#[test]
fn classify_update_virtual_catalog() {
    let sql = "UPDATE rocklake_catalog.ducklake_table SET end_snapshot = 7 WHERE table_id = 1";
    let kind = classify_statement(sql).unwrap();
    assert!(
        matches!(kind, StatementKind::VirtualCatalogMutation { .. }),
        "UPDATE on rocklake_catalog.* must be VirtualCatalogMutation; got {kind:?}"
    );
}

// ─── DELETE ───────────────────────────────────────────────────────────────────

#[test]
fn classify_delete_virtual_catalog() {
    let sql = "DELETE FROM rocklake_catalog.ducklake_snapshot WHERE snapshot_id = 1";
    let kind = classify_statement(sql).unwrap();
    assert!(
        matches!(kind, StatementKind::VirtualCatalogMutation { .. }),
        "DELETE on rocklake_catalog.* must be VirtualCatalogMutation; got {kind:?}"
    );
}

// ─── SELECT still works ───────────────────────────────────────────────────────

#[test]
fn classify_select_virtual_catalog_is_not_mutation() {
    let sql = "SELECT * FROM rocklake_catalog.ducklake_snapshot";
    let kind = classify_statement(sql).unwrap();
    assert!(
        matches!(kind, StatementKind::VirtualCatalogScan { .. }),
        "SELECT from rocklake_catalog.* must be VirtualCatalogScan; got {kind:?}"
    );
}
