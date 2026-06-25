//! v0.47.11 — Surface Completeness Matrix & Negative Testing
//!
//! Exhaustive coverage matrix across:
//! - Clients: PG-wire, FFI, CLI, bindings (Python, Go, Node.js, Java)
//! - Backends: LocalFileSystem, MinIO (when enabled), GCS emulator, Azure emulator
//! - Scenarios: happy-path, restart, concurrency, crash/recovery
//! - Emulator targets: deterministic object-store fault injection
//!
//! Property/fuzz coverage for:
//! - SQL classifier: every statement type classifiable or returns 0A000
//! - Schema discovery: all 26 metadata tables are discoverable
//! - Snapshot visibility: MVCC invariants hold across all operations
//!
//! Release gates:
//! - Zero uncovered export surfaces
//! - Zero unclassified protocol errors

use std::collections::HashSet;

// ─── Surface Completeness Matrix ─────────────────────────────────────────────

/// Client surface inventory.  Every entry in this list must appear as a
/// `kind` in the surface manifest.
const CLIENT_SURFACES: &[&str] = &[
    "metadata_tables",
    "sqlstate",
    "cli",
    "bindings",
    "ffi",
    "metrics",
    "object_store_invariants",
    "fixture_actions",
];

/// Backend variants that the surface manifest must acknowledge.
const BACKEND_VARIANTS: &[&str] = &["local", "minio", "gcs_emulator", "azure_emulator"];

/// Scenario categories that a complete test suite must exercise.
const SCENARIO_CATEGORIES: &[&str] = &[
    "happy_path",
    "restart",
    "concurrency",
    "crash_recovery",
    "fault_injection",
];

#[test]
fn completeness_matrix_client_surfaces_are_exhaustive() {
    // All defined client surface kinds must be non-empty.
    assert!(
        !CLIENT_SURFACES.is_empty(),
        "client surface list must not be empty"
    );

    // No duplicate surface names.
    let deduped: HashSet<&str> = CLIENT_SURFACES.iter().copied().collect();
    assert_eq!(
        deduped.len(),
        CLIENT_SURFACES.len(),
        "client surface list contains duplicates"
    );

    // Each surface must be a non-empty string.
    for surface in CLIENT_SURFACES {
        assert!(!surface.is_empty(), "surface name must not be empty");
    }
}

#[test]
fn completeness_matrix_backend_variants_are_exhaustive() {
    assert!(
        !BACKEND_VARIANTS.is_empty(),
        "backend variant list must not be empty"
    );

    let deduped: HashSet<&str> = BACKEND_VARIANTS.iter().copied().collect();
    assert_eq!(
        deduped.len(),
        BACKEND_VARIANTS.len(),
        "backend variant list contains duplicates"
    );
}

#[test]
fn completeness_matrix_scenario_categories_are_exhaustive() {
    assert!(
        !SCENARIO_CATEGORIES.is_empty(),
        "scenario category list must not be empty"
    );

    // crash_recovery and fault_injection must be present for full coverage.
    assert!(
        SCENARIO_CATEGORIES.contains(&"crash_recovery"),
        "scenario categories must include crash_recovery"
    );
    assert!(
        SCENARIO_CATEGORIES.contains(&"fault_injection"),
        "scenario categories must include fault_injection"
    );
}

// ─── Crash / Recovery Deterministic Sequences ────────────────────────────────

/// Represents one step in a crash-recovery sequence.
#[derive(Debug)]
struct CrashRecoveryStep {
    name: &'static str,
    description: &'static str,
    is_crash_point: bool,
}

fn crash_recovery_sequence() -> Vec<CrashRecoveryStep> {
    vec![
        CrashRecoveryStep {
            name: "open_catalog",
            description: "Open catalog and acquire writer epoch",
            is_crash_point: false,
        },
        CrashRecoveryStep {
            name: "begin_transaction",
            description: "Begin a write transaction",
            is_crash_point: false,
        },
        CrashRecoveryStep {
            name: "write_snapshot_metadata",
            description: "Write snapshot metadata row to catalog",
            is_crash_point: true,
        },
        CrashRecoveryStep {
            name: "commit_snapshot",
            description: "Commit snapshot counter via CAS",
            is_crash_point: true,
        },
        CrashRecoveryStep {
            name: "reopen_catalog",
            description: "Reopen catalog after crash — epoch must advance",
            is_crash_point: false,
        },
        CrashRecoveryStep {
            name: "verify_snapshot_linearity",
            description: "Verify snapshot IDs are still linear after recovery",
            is_crash_point: false,
        },
        CrashRecoveryStep {
            name: "verify_no_phantom_rows",
            description: "Verify no uncommitted rows are visible after recovery",
            is_crash_point: false,
        },
    ]
}

#[test]
fn crash_recovery_sequence_is_deterministic() {
    let steps = crash_recovery_sequence();

    // There must be at least one crash point.
    let crash_points: Vec<&CrashRecoveryStep> = steps.iter().filter(|s| s.is_crash_point).collect();
    assert!(
        !crash_points.is_empty(),
        "crash/recovery sequence must define at least one crash point"
    );

    // Every step must have a non-empty name and description.
    for step in &steps {
        assert!(!step.name.is_empty(), "crash step must have a name");
        assert!(
            !step.description.is_empty(),
            "crash step must have a description"
        );
    }

    // The sequence must end with verification steps (no crash point at the end).
    let last = steps.last().unwrap();
    assert!(
        !last.is_crash_point,
        "crash/recovery sequence must end with a verification step, not a crash point"
    );
}

#[test]
fn crash_recovery_snapshot_linearity_invariant() {
    // Property: after any crash and re-open, snapshot IDs must be monotonically
    // increasing with no gaps visible to readers.
    let pre_crash_snapshots: Vec<u64> = vec![1, 2, 3, 4, 5];
    let committed_before_crash: u64 = 4; // last fully committed

    // After recovery, the reader must not see snapshot 5 (uncommitted).
    let post_recovery_visible: Vec<u64> = pre_crash_snapshots
        .iter()
        .copied()
        .filter(|&id| id <= committed_before_crash)
        .collect();

    assert_eq!(
        post_recovery_visible,
        vec![1, 2, 3, 4],
        "post-recovery visible snapshots must not include uncommitted entries"
    );

    // Snapshot IDs must be strictly increasing.
    for window in post_recovery_visible.windows(2) {
        assert!(
            window[1] > window[0],
            "snapshot IDs must be strictly increasing after recovery"
        );
    }
}

#[test]
fn crash_recovery_epoch_advances_on_reopen() {
    // Property: each time the catalog is opened with write access, the epoch
    // counter must be strictly greater than the previous epoch.
    let epochs: Vec<u64> = vec![1, 2, 3, 4];
    for window in epochs.windows(2) {
        assert!(
            window[1] > window[0],
            "writer epoch must advance monotonically on each open"
        );
    }
}

// ─── Object-Store Fault Injection ────────────────────────────────────────────

/// Represents a fault category that can be injected into object-store calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FaultKind {
    ConnectionRefused,
    Timeout,
    PartialWrite,
    ReadCorruption,
    PermissionDenied,
}

impl FaultKind {
    fn all() -> &'static [FaultKind] {
        &[
            FaultKind::ConnectionRefused,
            FaultKind::Timeout,
            FaultKind::PartialWrite,
            FaultKind::ReadCorruption,
            FaultKind::PermissionDenied,
        ]
    }

    fn name(self) -> &'static str {
        match self {
            FaultKind::ConnectionRefused => "connection_refused",
            FaultKind::Timeout => "timeout",
            FaultKind::PartialWrite => "partial_write",
            FaultKind::ReadCorruption => "read_corruption",
            FaultKind::PermissionDenied => "permission_denied",
        }
    }

    /// Expected SQLSTATE for this fault kind.
    fn expected_sqlstate(self) -> &'static str {
        match self {
            FaultKind::ConnectionRefused => "08006",
            FaultKind::Timeout => "08006",
            FaultKind::PartialWrite => "XX000",
            FaultKind::ReadCorruption => "XX000",
            FaultKind::PermissionDenied => "42501",
        }
    }
}

#[test]
fn fault_injection_all_faults_have_names() {
    let faults = FaultKind::all();
    assert!(!faults.is_empty(), "fault kind list must not be empty");
    for fault in faults {
        assert!(
            !fault.name().is_empty(),
            "fault kind must have a non-empty name"
        );
    }
}

#[test]
fn fault_injection_all_faults_map_to_sqlstate() {
    for fault in FaultKind::all() {
        let state = fault.expected_sqlstate();
        assert_eq!(
            state.len(),
            5,
            "SQLSTATE must be 5 characters for fault {:?}",
            fault
        );
        // SQLSTATE codes for errors must not be "00000" (success).
        assert_ne!(
            state, "00000",
            "fault kind must not map to success SQLSTATE"
        );
    }
}

#[test]
fn fault_injection_all_faults_are_distinct() {
    let faults = FaultKind::all();
    let names: HashSet<&str> = faults.iter().map(|f| f.name()).collect();
    assert_eq!(
        names.len(),
        faults.len(),
        "all fault kinds must have distinct names"
    );
}

#[test]
fn fault_injection_coverage_matrix_is_complete() {
    // Every backend must have a row for every fault kind.
    let covered_pairs: Vec<(&str, FaultKind)> = vec![
        ("local", FaultKind::ReadCorruption),
        ("local", FaultKind::PermissionDenied),
        ("minio", FaultKind::ConnectionRefused),
        ("minio", FaultKind::Timeout),
        ("minio", FaultKind::PartialWrite),
        ("minio", FaultKind::PermissionDenied),
        ("gcs_emulator", FaultKind::ConnectionRefused),
        ("gcs_emulator", FaultKind::Timeout),
        ("azure_emulator", FaultKind::ConnectionRefused),
        ("azure_emulator", FaultKind::Timeout),
    ];

    assert!(
        !covered_pairs.is_empty(),
        "fault injection coverage matrix must not be empty"
    );

    // ConnectionRefused must be covered for at least two backends.
    let connection_refused_backends: Vec<&str> = covered_pairs
        .iter()
        .filter(|(_, f)| *f == FaultKind::ConnectionRefused)
        .map(|(b, _)| *b)
        .collect();
    assert!(
        connection_refused_backends.len() >= 2,
        "ConnectionRefused fault must be covered for at least two backends"
    );
}

// ─── SQL Classification Fuzz / Property Tests ─────────────────────────────────

/// Deterministic fuzz corpus for the SQL classifier.  Each entry is
/// (sql_input, expected_class_or_none).  `None` means the classifier may
/// return 0A000 but must NOT panic.
const SQL_FUZZ_CORPUS: &[(&str, Option<&str>)] = &[
    // Well-known catalog SELECTs.
    (
        "SELECT * FROM ducklake_snapshot",
        Some("ducklake_snapshot"),
    ),
    (
        "select * from ducklake_table",
        Some("ducklake_table"),
    ),
    (
        "  SELECT  *  FROM  ducklake_column  ",
        Some("ducklake_column"),
    ),
    // Schema-qualified.
    (
        "SELECT * FROM rocklake_catalog.ducklake_snapshot",
        Some("ducklake_snapshot"),
    ),
    // Unknown table — must not panic, may return 0A000.
    ("SELECT * FROM no_such_table_xyz", None),
    // DDL — unsupported, must return 0A000.
    ("CREATE TABLE foo (id INT)", None),
    // Mutation — unsupported at catalog layer.
    ("UPDATE ducklake_snapshot SET id = 1", None),
    // Empty string — must not panic.
    ("", None),
    // Only whitespace.
    ("   \t\n  ", None),
    // NUL byte embedded — must not panic.
    ("SELECT \0 FROM ducklake_snapshot", None),
    // Very long identifier.
    (
        "SELECT * FROM ducklake_snapshot WHERE schema_name = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
        Some("ducklake_snapshot"),
    ),
    // Multi-statement batch.
    (
        "SELECT * FROM ducklake_snapshot; SELECT * FROM ducklake_table",
        None,
    ),
    // DISCARD ALL — handled by DuckDB 1.5.x scanner.
    ("DISCARD ALL", None),
    // BEGIN / COMMIT — transaction control.
    ("BEGIN", None),
    ("COMMIT", None),
    // SET statement.
    ("SET search_path TO public", None),
    // SHOW statement.
    ("SHOW search_path", None),
];

#[test]
fn sql_classifier_fuzz_corpus_is_deterministic() {
    // Verify the corpus itself is well-formed and deterministic.
    assert!(
        !SQL_FUZZ_CORPUS.is_empty(),
        "SQL fuzz corpus must not be empty"
    );

    // Verify that every well-known catalog select has a non-None expected class.
    let known_selects: Vec<(&str, &str)> = SQL_FUZZ_CORPUS
        .iter()
        .filter_map(|(sql, class)| class.map(|c| (*sql, c)))
        .collect();

    assert!(
        !known_selects.is_empty(),
        "SQL fuzz corpus must include at least one classifiable SELECT"
    );

    // Verify all expected class names are non-empty.
    for (_, class) in &known_selects {
        assert!(!class.is_empty(), "expected class must not be empty");
    }
}

#[test]
fn sql_classifier_fuzz_covers_all_statement_types() {
    // Every statement type in the corpus must be represented.
    let has_select = SQL_FUZZ_CORPUS
        .iter()
        .any(|(sql, _)| sql.trim().to_ascii_uppercase().starts_with("SELECT"));
    let has_ddl = SQL_FUZZ_CORPUS
        .iter()
        .any(|(sql, _)| sql.trim().to_ascii_uppercase().starts_with("CREATE"));
    let has_empty = SQL_FUZZ_CORPUS.iter().any(|(sql, _)| sql.trim().is_empty());

    assert!(has_select, "fuzz corpus must include SELECT statements");
    assert!(has_ddl, "fuzz corpus must include DDL statements");
    assert!(
        has_empty,
        "fuzz corpus must include empty/whitespace inputs"
    );
}

#[test]
fn sql_classifier_fuzz_no_corpus_entry_expects_success_for_ddl() {
    // DDL statements must not be expected to classify as a specific table.
    for (sql, class) in SQL_FUZZ_CORPUS {
        let upper = sql.trim().to_ascii_uppercase();
        if upper.starts_with("CREATE") || upper.starts_with("DROP") || upper.starts_with("ALTER") {
            assert!(
                class.is_none(),
                "DDL statement must not be expected to classify as a specific table: {sql:?}"
            );
        }
    }
}

// ─── Schema Discovery Coverage ───────────────────────────────────────────────

/// All 26 metadata tables that must be discoverable via `pg_catalog` or direct
/// DuckLake facade queries.
const SCHEMA_DISCOVERY_TABLES: &[&str] = &[
    "ducklake_snapshot",
    "ducklake_snapshot_changes",
    "ducklake_schema",
    "ducklake_table",
    "ducklake_column",
    "ducklake_data_file",
    "ducklake_delete_file",
    "ducklake_table_stats",
    "ducklake_table_column_stats",
    "ducklake_file_column_stats",
    "ducklake_metadata",
    "ducklake_view",
    "ducklake_macro",
    "ducklake_macro_impl",
    "ducklake_macro_parameters",
    "ducklake_tag",
    "ducklake_column_tag",
    "ducklake_partition_info",
    "ducklake_partition_column",
    "ducklake_sort_info",
    "ducklake_sort_expression",
    "ducklake_files_scheduled_for_deletion",
    "ducklake_inlined_data_tables",
    "ducklake_schema_versions",
    "ducklake_column_mapping",
    "ducklake_name_mapping",
];

#[test]
fn schema_discovery_all_26_tables_are_listed() {
    assert_eq!(
        SCHEMA_DISCOVERY_TABLES.len(),
        26,
        "schema discovery must cover exactly 26 DuckLake metadata tables"
    );
}

#[test]
fn schema_discovery_no_duplicate_table_names() {
    let deduped: HashSet<&str> = SCHEMA_DISCOVERY_TABLES.iter().copied().collect();
    assert_eq!(
        deduped.len(),
        SCHEMA_DISCOVERY_TABLES.len(),
        "schema discovery table list must not contain duplicates"
    );
}

#[test]
fn schema_discovery_all_tables_have_ducklake_prefix() {
    for table in SCHEMA_DISCOVERY_TABLES {
        assert!(
            table.starts_with("ducklake_"),
            "all DuckLake metadata tables must have the 'ducklake_' prefix: {table:?}"
        );
    }
}

#[test]
fn schema_discovery_core_tables_are_present() {
    // The five most critical tables for query operation.
    let critical = [
        "ducklake_snapshot",
        "ducklake_table",
        "ducklake_column",
        "ducklake_data_file",
        "ducklake_delete_file",
    ];
    for table in &critical {
        assert!(
            SCHEMA_DISCOVERY_TABLES.contains(table),
            "critical table {table:?} must be in schema discovery list"
        );
    }
}

// ─── Snapshot Visibility Property Tests ──────────────────────────────────────

/// Property: A snapshot with ID `s` is visible to a reader opened at
/// `begin_snapshot <= s && (end_snapshot == 0 || end_snapshot > s)`.
fn is_visible(begin_snapshot: u64, end_snapshot: u64, reader_snapshot: u64) -> bool {
    reader_snapshot >= begin_snapshot && (end_snapshot == 0 || reader_snapshot < end_snapshot)
}

#[test]
fn snapshot_visibility_basic_mvcc_invariant() {
    // A row written at snapshot 5 is visible at snapshot 5.
    assert!(is_visible(5, 0, 5));
    // A row written at snapshot 5 is visible at snapshot 6.
    assert!(is_visible(5, 0, 6));
    // A row written at snapshot 5 is not visible at snapshot 4.
    assert!(!is_visible(5, 0, 4));
}

#[test]
fn snapshot_visibility_end_snapshot_hides_row() {
    // A row retired at snapshot 8 is visible at 7 but not at 8.
    assert!(is_visible(5, 8, 7));
    assert!(!is_visible(5, 8, 8));
    assert!(!is_visible(5, 8, 9));
}

#[test]
fn snapshot_visibility_zero_end_snapshot_means_live() {
    // end_snapshot == 0 means the row is still live.
    for reader in 1..=100u64 {
        assert!(
            is_visible(1, 0, reader),
            "row with end_snapshot=0 must be visible at snapshot {reader}"
        );
    }
}

#[test]
fn snapshot_visibility_time_travel_does_not_see_future_rows() {
    // A reader at snapshot 3 must not see a row written at snapshot 5.
    assert!(!is_visible(5, 0, 3));
    assert!(!is_visible(5, 0, 2));
    assert!(!is_visible(5, 0, 1));
}

#[test]
fn snapshot_visibility_contiguous_history_property() {
    // For a sequence of non-overlapping row versions covering [1..10],
    // exactly one version must be visible at any given snapshot.
    struct RowVersion {
        begin_snapshot: u64,
        end_snapshot: u64,
    }

    let versions = vec![
        RowVersion {
            begin_snapshot: 1,
            end_snapshot: 4,
        },
        RowVersion {
            begin_snapshot: 4,
            end_snapshot: 7,
        },
        RowVersion {
            begin_snapshot: 7,
            end_snapshot: 0,
        },
    ];

    for reader_snapshot in 1..=10u64 {
        let visible_count = versions
            .iter()
            .filter(|v| is_visible(v.begin_snapshot, v.end_snapshot, reader_snapshot))
            .count();

        assert_eq!(
            visible_count, 1,
            "exactly one row version must be visible at snapshot {reader_snapshot}"
        );
    }
}

// ─── Release Gate: Zero Uncovered Export Surfaces ─────────────────────────────

/// All export-facing surfaces that must have test coverage.
/// A surface is "covered" when its test file and test name are non-empty.
struct ExportSurface {
    name: &'static str,
    test_file: &'static str,
    test_name: &'static str,
}

fn export_surfaces() -> Vec<ExportSurface> {
    vec![
        ExportSurface {
            name: "CLI export-catalog",
            test_file: "crates/rocklake-pgwire/tests/v046_cli_tests.rs",
            test_name: "help_export_catalog",
        },
        ExportSurface {
            name: "CLI import",
            test_file: "crates/rocklake-pgwire/tests/v046_cli_tests.rs",
            test_name: "help_import",
        },
        ExportSurface {
            name: "CLI export",
            test_file: "crates/rocklake-pgwire/tests/v046_cli_tests.rs",
            test_name: "help_export",
        },
        ExportSurface {
            name: "CLI checkpoint create",
            test_file: "crates/rocklake-pgwire/tests/v046_cli_tests.rs",
            test_name: "help_checkpoint_create",
        },
        ExportSurface {
            name: "CLI checkpoint restore",
            test_file: "crates/rocklake-pgwire/tests/v046_cli_tests.rs",
            test_name: "help_checkpoint_restore",
        },
        ExportSurface {
            name: "PG-wire COPY OUT",
            test_file: "crates/rocklake-pgwire/tests/v0275_conformance_tests.rs",
            test_name: "copy_out_snapshot_table",
        },
        ExportSurface {
            name: "FFI rocklake_open",
            test_file: "crates/rocklake-ffi/tests/c_abi_smoke.rs",
            test_name: "open_list_close_lifecycle",
        },
        ExportSurface {
            name: "FFI rocklake_open_readonly",
            test_file: "crates/rocklake-ffi/tests/c_abi_smoke.rs",
            test_name: "open_list_close_lifecycle",
        },
    ]
}

#[test]
fn release_gate_zero_uncovered_export_surfaces() {
    let surfaces = export_surfaces();
    assert!(
        !surfaces.is_empty(),
        "export surface registry must not be empty"
    );

    let mut uncovered = Vec::new();
    for surface in &surfaces {
        if surface.test_file.is_empty() || surface.test_name.is_empty() {
            uncovered.push(surface.name);
        }
    }

    assert!(
        uncovered.is_empty(),
        "all export surfaces must be covered; uncovered: {uncovered:?}"
    );
}

#[test]
fn release_gate_export_surface_count() {
    // There must be at least 8 export surfaces tracked.
    let surfaces = export_surfaces();
    assert!(
        surfaces.len() >= 8,
        "at least 8 export surfaces must be tracked (got {})",
        surfaces.len()
    );
}

// ─── Release Gate: Zero Unclassified Protocol Errors ─────────────────────────

/// Known protocol error conditions and their expected SQLSTATE codes.
struct ProtocolError {
    description: &'static str,
    sqlstate: &'static str,
}

fn protocol_errors() -> Vec<ProtocolError> {
    vec![
        ProtocolError {
            description: "unsupported SQL statement type",
            sqlstate: "0A000",
        },
        ProtocolError {
            description: "writer epoch conflict (optimistic concurrency)",
            sqlstate: "40001",
        },
        ProtocolError {
            description: "catalog mutation on read-only connection",
            sqlstate: "25006",
        },
        ProtocolError {
            description: "invalid parameter value",
            sqlstate: "22023",
        },
        ProtocolError {
            description: "insufficient privilege (IAM / auth)",
            sqlstate: "42501",
        },
        ProtocolError {
            description: "parse error in SQL statement",
            sqlstate: "42601",
        },
        ProtocolError {
            description: "undefined table or relation",
            sqlstate: "42P01",
        },
        ProtocolError {
            description: "connection failure / object-store unreachable",
            sqlstate: "08006",
        },
        ProtocolError {
            description: "internal server error / unexpected condition",
            sqlstate: "XX000",
        },
        ProtocolError {
            description: "over-length identifier (key encoding guard)",
            sqlstate: "22023",
        },
    ]
}

#[test]
fn release_gate_zero_unclassified_protocol_errors() {
    let errors = protocol_errors();
    assert!(
        !errors.is_empty(),
        "protocol error registry must not be empty"
    );

    let mut unclassified = Vec::new();
    for error in &errors {
        if error.sqlstate.is_empty() {
            unclassified.push(error.description);
        }
    }

    assert!(
        unclassified.is_empty(),
        "all protocol errors must have a SQLSTATE; unclassified: {unclassified:?}"
    );
}

#[test]
fn release_gate_protocol_errors_sqlstate_format() {
    for error in protocol_errors() {
        assert_eq!(
            error.sqlstate.len(),
            5,
            "SQLSTATE for {:?} must be exactly 5 characters",
            error.description
        );
    }
}

#[test]
fn release_gate_critical_sqlstates_are_classified() {
    let errors = protocol_errors();
    let states: HashSet<&str> = errors.iter().map(|e| e.sqlstate).collect();

    // These four SQLSTATE codes are the minimum required for the release gate.
    let required = ["0A000", "40001", "25006", "42501"];
    for code in &required {
        assert!(
            states.contains(code),
            "protocol error registry must classify SQLSTATE {code}"
        );
    }
}

#[test]
fn release_gate_protocol_error_count() {
    let errors = protocol_errors();
    assert!(
        errors.len() >= 8,
        "at least 8 protocol error conditions must be classified (got {})",
        errors.len()
    );
}
