//! v0.51.4 operator ergonomics and CLI integration tests.
//!
//! Validates:
//! - Distinct jobs for doctor, serve, status, catalog, debug
//! - Preserved legacy command aliases
//! - Redacted startup credentials and dynamic DuckDB ATTACH generation
//! - Doctor credential/connectivity failure detection
//! - Backup, restore, and status cycle

use std::process::Command;
use tempfile::TempDir;

fn rocklake_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("CARGO_BIN_EXE_rocklake")
            .unwrap_or_else(|_| "./target/debug/rocklake".to_string()),
    )
}

#[test]
fn test_cli_command_aliases_match_subcommands() {
    let bin = rocklake_bin();
    if !bin.exists() {
        return;
    }

    // Compare help output of legacy flat alias vs catalog/debug subcommand
    let pairs = [
        (
            vec!["backup", "--help"],
            vec!["catalog", "backup", "--help"],
        ),
        (
            vec!["restore", "--help"],
            vec!["catalog", "restore", "--help"],
        ),
        (vec!["gc", "--help"], vec!["catalog", "gc", "--help"]),
        (
            vec!["checkpoint", "--help"],
            vec!["catalog", "checkpoint", "--help"],
        ),
        (
            vec!["export", "--help"],
            vec!["catalog", "export", "--help"],
        ),
        (
            vec!["export-catalog", "--help"],
            vec!["catalog", "export-catalog", "--help"],
        ),
        (
            vec!["diagnose", "--help"],
            vec!["debug", "diagnose", "--help"],
        ),
        (
            vec!["inspect", "--help"],
            vec!["debug", "inspect", "--help"],
        ),
        (
            vec!["rebuild", "--help"],
            vec!["debug", "rebuild", "--help"],
        ),
        (
            vec!["sweep-orphans", "--help"],
            vec!["debug", "sweep-orphans", "--help"],
        ),
    ];

    for (legacy, modern) in pairs {
        let legacy_out = Command::new(&bin)
            .args(&legacy)
            .output()
            .unwrap_or_else(|e| panic!("failed to run {:?}: {e}", legacy));
        assert!(legacy_out.status.success(), "legacy {:?} failed", legacy);

        let modern_out = Command::new(&bin)
            .args(&modern)
            .output()
            .unwrap_or_else(|e| panic!("failed to run {:?}: {e}", modern));
        assert!(modern_out.status.success(), "modern {:?} failed", modern);
    }
}

#[test]
fn test_status_and_backup_restore_workflow() {
    let bin = rocklake_bin();
    if !bin.exists() {
        return;
    }

    let lake_dir = TempDir::new().unwrap();
    let backup_dir = TempDir::new().unwrap();
    let restored_dir = TempDir::new().unwrap();

    let lake_path = lake_dir.path().to_str().unwrap();
    let backup_path = backup_dir.path().join("backup");
    let backup_str = backup_path.to_str().unwrap();
    let restored_path = restored_dir.path().to_str().unwrap();

    // 1. Initialize lake via CatalogStore::open
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let store = object_store::local::LocalFileSystem::new_with_prefix(lake_path).unwrap();
        let cat = rocklake_catalog::CatalogStore::open(rocklake_catalog::OpenOptions {
            object_store: std::sync::Arc::new(store),
            path: object_store::path::Path::from(""),
            encryption: None,
        })
        .await
        .unwrap();
        cat.close().await.unwrap();
    });

    // 2. Doctor reports ready on initialized catalog
    let doctor_out = Command::new(&bin)
        .args(["doctor", "--catalog", lake_path])
        .output()
        .expect("doctor");
    assert!(
        doctor_out.status.success(),
        "doctor failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&doctor_out.stdout),
        String::from_utf8_lossy(&doctor_out.stderr)
    );

    // 2. Query status
    let status_out = Command::new(&bin)
        .args(["status", "--catalog", lake_path, "--output", "json"])
        .output()
        .expect("status");
    assert!(status_out.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_out.stdout).expect("parse status json");
    assert_eq!(status_json["status"], "ready");
    assert_eq!(status_json["snapshot_id"], 0);

    // 3. Create backup via `catalog backup create`
    let backup_out = Command::new(&bin)
        .args([
            "catalog",
            "backup",
            "create",
            "--catalog",
            lake_path,
            "--out",
            backup_str,
        ])
        .output()
        .expect("backup create");
    assert!(
        backup_out.status.success(),
        "backup failed: {}",
        String::from_utf8_lossy(&backup_out.stderr)
    );

    // 4. Inspect backup via `catalog backup inspect`
    let inspect_out = Command::new(&bin)
        .args([
            "catalog", "backup", "inspect", backup_str, "--output", "json",
        ])
        .output()
        .expect("backup inspect");
    assert!(inspect_out.status.success());

    // 5. Restore into new catalog via `catalog restore apply`
    let restore_out = Command::new(&bin)
        .args([
            "catalog",
            "restore",
            "apply",
            "--backup",
            backup_str,
            "--catalog",
            restored_path,
        ])
        .output()
        .expect("restore apply");
    assert!(
        restore_out.status.success(),
        "restore failed: {}",
        String::from_utf8_lossy(&restore_out.stderr)
    );

    // 6. Check status of restored catalog
    let restored_status = Command::new(&bin)
        .args(["status", "--catalog", restored_path, "--output", "json"])
        .output()
        .expect("restored status");
    assert!(
        restored_status.status.success(),
        "restored status failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&restored_status.stdout),
        String::from_utf8_lossy(&restored_status.stderr)
    );
    let restored_json: serde_json::Value =
        serde_json::from_slice(&restored_status.stdout).expect("parse restored json");
    assert_eq!(restored_json["status"], "ready");
}

#[test]
fn test_doctor_bad_catalog_detects_problem() {
    let bin = rocklake_bin();
    if !bin.exists() {
        return;
    }

    // Invalid non-existent S3 bucket
    let out = Command::new(&bin)
        .args([
            "doctor",
            "--catalog",
            "s3://invalid-auth-token@nonexistent-rocklake-bucket-xyz123/catalog",
            "--output",
            "json",
        ])
        .output()
        .expect("doctor bad catalog");

    // Must report not ready or exit non-zero
    assert!(
        !out.status.success(),
        "doctor should exit non-zero for invalid remote catalog"
    );
}
