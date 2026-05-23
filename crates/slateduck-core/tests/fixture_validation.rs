//! Integration tests for wire corpus fixture validation.

use std::fs;
use std::path::Path;

#[test]
fn wire_corpus_fixtures_are_valid_jsonl() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/wire-corpus");

    assert!(
        fixtures_dir.exists(),
        "wire-corpus fixtures directory must exist"
    );

    for entry in fs::read_dir(&fixtures_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            let content = fs::read_to_string(&path).unwrap();
            for (line_num, line) in content.lines().enumerate() {
                let parsed: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| {
                    panic!("Invalid JSON at {}:{}: {}", path.display(), line_num + 1, e)
                });
                // Each line must be a JSON object
                assert!(
                    parsed.is_object(),
                    "Line {} in {} is not a JSON object",
                    line_num + 1,
                    path.display()
                );
            }
        }
    }
}

#[test]
fn handshake_fixtures_are_valid_jsonl() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/handshake");

    assert!(
        fixtures_dir.exists(),
        "handshake fixtures directory must exist"
    );

    for entry in fs::read_dir(&fixtures_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            let content = fs::read_to_string(&path).unwrap();
            for (line_num, line) in content.lines().enumerate() {
                let parsed: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| {
                    panic!("Invalid JSON at {}:{}: {}", path.display(), line_num + 1, e)
                });
                assert!(parsed.is_object());
            }
        }
    }
}

#[test]
fn golden_fixtures_exist() {
    let golden_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/golden/duckdb-1.5.2");

    assert!(golden_dir.exists(), "golden fixtures directory must exist");
    assert!(golden_dir.join("tutorial-output.sql").exists());
    assert!(golden_dir.join("tutorial-state.json").exists());

    // Validate tutorial-state.json is valid JSON
    let state = fs::read_to_string(golden_dir.join("tutorial-state.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert!(parsed.is_object());
    assert!(parsed.get("tutorial_steps").unwrap().is_array());
}

#[test]
fn wire_corpus_has_required_message_types() {
    let corpus_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/wire-corpus/duckdb-1.5.2.jsonl");

    let content = fs::read_to_string(&corpus_path).unwrap();

    // Verify required message types are present
    let required_types = vec![
        "startup",
        "authentication_ok",
        "parameter_status",
        "ready_for_query",
        "query",
        "row_description",
        "data_row",
        "command_complete",
    ];

    for msg_type in &required_types {
        assert!(
            content.contains(&format!("\"type\":\"{}\"", msg_type)),
            "Wire corpus missing required message type: {}",
            msg_type
        );
    }

    // Verify corpus includes DuckLake-specific operations
    assert!(
        content.contains("ducklake_snapshot"),
        "Missing snapshot operations"
    );
    assert!(
        content.contains("ducklake_schema"),
        "Missing schema operations"
    );
    assert!(
        content.contains("ducklake_table"),
        "Missing table operations"
    );
    assert!(
        content.contains("ducklake_column"),
        "Missing column operations"
    );
    assert!(content.contains("BEGIN"), "Missing BEGIN transaction");
    assert!(content.contains("COMMIT"), "Missing COMMIT transaction");
    assert!(
        content.contains("current_schema"),
        "Missing current_schema probe"
    );
    assert!(
        content.contains("pg_catalog.pg_type"),
        "Missing pg_type probe"
    );
    assert!(content.contains("SET"), "Missing SET statement");
}

#[test]
fn handshake_corpus_complete() {
    let handshake_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/handshake/duckdb-1.5.2.jsonl");

    let content = fs::read_to_string(&handshake_path).unwrap();

    // Must include startup, auth, parameter_status, and type probes
    assert!(content.contains("\"type\":\"startup\""));
    assert!(content.contains("\"type\":\"authentication_ok\""));
    assert!(content.contains("\"type\":\"parameter_status\""));
    assert!(content.contains("pg_catalog.pg_type"));
}
