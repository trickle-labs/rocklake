//! Schema evolution detection for materialized views.
//!
//! Tracks base table schema changes and determines view staleness/brokenness.

use serde::{Deserialize, Serialize};

/// View status after schema evolution check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewStatus {
    /// View is fresh and valid.
    Fresh,
    /// View is stale due to schema change; `REFRESH ... FULL` can recover it.
    Stale { reason: String },
    /// View is broken due to a dropped referenced column; must be re-created.
    Broken { reason: String },
}

/// A schema change event on a base table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaChange {
    /// A new column was added.
    AddColumn { column: String },
    /// A column's type was changed.
    ChangeType { column: String, new_type: String },
    /// A column was renamed.
    RenameColumn { old_name: String, new_name: String },
    /// A column was dropped.
    DropColumn { column: String },
}

/// Check how a schema change affects a materialized view.
///
/// `referenced_columns` is the set of columns the view's SQL references.
pub fn check_schema_impact(change: &SchemaChange, referenced_columns: &[String]) -> ViewStatus {
    match change {
        SchemaChange::AddColumn { column } => {
            if referenced_columns.contains(column) {
                ViewStatus::Stale {
                    reason: format!(
                        "Column '{}' added to base table and is referenced by view; REFRESH FULL required",
                        column
                    ),
                }
            } else {
                // Column not referenced — no-op.
                ViewStatus::Fresh
            }
        }
        SchemaChange::ChangeType { column, new_type } => {
            if referenced_columns.contains(column) {
                ViewStatus::Stale {
                    reason: format!(
                        "Column '{}' type changed to '{}'; REFRESH FULL required",
                        column, new_type
                    ),
                }
            } else {
                ViewStatus::Fresh
            }
        }
        SchemaChange::RenameColumn { old_name, .. } => {
            if referenced_columns.contains(old_name) {
                ViewStatus::Stale {
                    reason: format!(
                        "Column '{}' renamed; view must be dropped and re-created with corrected SQL",
                        old_name
                    ),
                }
            } else {
                ViewStatus::Fresh
            }
        }
        SchemaChange::DropColumn { column } => {
            if referenced_columns.contains(column) {
                ViewStatus::Broken {
                    reason: format!(
                        "Column '{}' dropped from base table; view SQL is un-parseable. \
                         DROP and re-create the view with corrected SQL",
                        column
                    ),
                }
            } else {
                ViewStatus::Fresh
            }
        }
    }
}

/// Format view status for `SHOW MATERIALIZED VIEWS`.
pub fn format_view_status(status: &ViewStatus) -> (&'static str, String) {
    match status {
        ViewStatus::Fresh => ("fresh", String::new()),
        ViewStatus::Stale { reason } => ("stale", reason.clone()),
        ViewStatus::Broken { reason } => ("broken", reason.clone()),
    }
}

/// Attempt a REFRESH FULL on a view with a given status.
/// Returns Ok(()) if recoverable, Err with message if broken.
pub fn attempt_refresh_full(status: &ViewStatus) -> Result<(), String> {
    match status {
        ViewStatus::Fresh => Ok(()),
        ViewStatus::Stale { .. } => Ok(()), // REFRESH FULL recovers stale views.
        ViewStatus::Broken { reason } => {
            Err(format!("Cannot REFRESH FULL: view is broken. {}", reason))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_column_not_referenced_is_noop() {
        let status = check_schema_impact(
            &SchemaChange::AddColumn {
                column: "new_col".to_string(),
            },
            &["dept".to_string(), "salary".to_string()],
        );
        assert_eq!(status, ViewStatus::Fresh);
    }

    #[test]
    fn add_column_referenced_is_stale() {
        let status = check_schema_impact(
            &SchemaChange::AddColumn {
                column: "salary".to_string(),
            },
            &["dept".to_string(), "salary".to_string()],
        );
        assert!(matches!(status, ViewStatus::Stale { .. }));
    }

    #[test]
    fn change_type_referenced_is_stale() {
        let status = check_schema_impact(
            &SchemaChange::ChangeType {
                column: "salary".to_string(),
                new_type: "VARCHAR".to_string(),
            },
            &["dept".to_string(), "salary".to_string()],
        );
        assert!(matches!(status, ViewStatus::Stale { .. }));
    }

    #[test]
    fn rename_column_referenced_is_stale() {
        let status = check_schema_impact(
            &SchemaChange::RenameColumn {
                old_name: "salary".to_string(),
                new_name: "compensation".to_string(),
            },
            &["dept".to_string(), "salary".to_string()],
        );
        assert!(matches!(status, ViewStatus::Stale { .. }));
    }

    #[test]
    fn drop_column_referenced_is_broken() {
        let status = check_schema_impact(
            &SchemaChange::DropColumn {
                column: "salary".to_string(),
            },
            &["dept".to_string(), "salary".to_string()],
        );
        assert!(matches!(status, ViewStatus::Broken { .. }));
        // Attempting REFRESH FULL on broken view should fail.
        let result = attempt_refresh_full(&status);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("salary"));
    }
}
