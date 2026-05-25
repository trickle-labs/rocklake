//! Tier 6d — Schema evolution tests.
//!
//! Tests: add-column no-op, add-referenced-column stale, type-change stale,
//! rename stale, drop-referenced broken + clear error on REFRESH FULL.

use slateduck_ivm::schema_evolution::{
    attempt_refresh_full, check_schema_impact, format_view_status, SchemaChange, ViewStatus,
};

/// Test: adding a column NOT referenced by the view is a no-op (view stays fresh).
#[test]
fn add_column_not_referenced_is_noop() {
    let referenced = vec!["dept".to_string(), "salary".to_string()];
    let change = SchemaChange::AddColumn {
        column: "hire_date".to_string(),
    };
    let status = check_schema_impact(&change, &referenced);
    assert_eq!(status, ViewStatus::Fresh);

    let (label, reason) = format_view_status(&status);
    assert_eq!(label, "fresh");
    assert!(reason.is_empty());
}

/// Test: adding a column referenced by the view marks it stale; REFRESH FULL recovers.
#[test]
fn add_column_referenced_is_stale_refresh_recovers() {
    let referenced = vec!["dept".to_string(), "salary".to_string()];
    let change = SchemaChange::AddColumn {
        column: "salary".to_string(),
    };
    let status = check_schema_impact(&change, &referenced);
    assert!(matches!(status, ViewStatus::Stale { .. }));

    let (label, reason) = format_view_status(&status);
    assert_eq!(label, "stale");
    assert!(reason.contains("salary"));
    assert!(reason.contains("REFRESH FULL"));

    // REFRESH FULL should recover.
    assert!(attempt_refresh_full(&status).is_ok());
}

/// Test: type-change on referenced column marks view stale.
#[test]
fn type_change_referenced_column_is_stale() {
    let referenced = vec!["dept".to_string(), "salary".to_string()];
    let change = SchemaChange::ChangeType {
        column: "salary".to_string(),
        new_type: "VARCHAR".to_string(),
    };
    let status = check_schema_impact(&change, &referenced);
    assert!(matches!(status, ViewStatus::Stale { .. }));

    let (label, _) = format_view_status(&status);
    assert_eq!(label, "stale");

    // REFRESH FULL recovers.
    assert!(attempt_refresh_full(&status).is_ok());
}

/// Test: renaming a referenced column marks view stale (re-creation required).
#[test]
fn rename_referenced_column_is_stale() {
    let referenced = vec!["dept".to_string(), "salary".to_string()];
    let change = SchemaChange::RenameColumn {
        old_name: "salary".to_string(),
        new_name: "compensation".to_string(),
    };
    let status = check_schema_impact(&change, &referenced);
    assert!(matches!(status, ViewStatus::Stale { .. }));

    if let ViewStatus::Stale { reason } = &status {
        assert!(reason.contains("salary"));
        assert!(reason.contains("re-created"));
    }
}

/// Test: dropping a referenced column marks view broken; REFRESH FULL returns clear error.
#[test]
fn drop_referenced_column_is_broken_refresh_full_errors() {
    let referenced = vec!["dept".to_string(), "salary".to_string()];
    let change = SchemaChange::DropColumn {
        column: "salary".to_string(),
    };
    let status = check_schema_impact(&change, &referenced);
    assert!(matches!(status, ViewStatus::Broken { .. }));

    let (label, reason) = format_view_status(&status);
    assert_eq!(label, "broken");
    assert!(reason.contains("salary"));
    assert!(reason.contains("dropped"));

    // REFRESH FULL should fail with clear error naming the missing column.
    let result = attempt_refresh_full(&status);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("broken"));
    assert!(err_msg.contains("salary"));
}
