use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn manifest_path() -> PathBuf {
    repo_root().join("tests/fixtures/public-surface-manifest.json")
}

fn load_json(path: &Path) -> Value {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

fn assert_path_exists(path: &Path) {
    assert!(path.exists(), "expected path to exist: {}", path.display());
}

fn assert_file_contains(path: &Path, needle: &str) {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    assert!(
        content.contains(needle),
        "expected {} to contain {needle:?}",
        path.display()
    );
}

fn assert_test_reference(root: &Path, test_file: &str, test_name: &str) {
    let test_path = root.join(test_file);
    assert_path_exists(&test_path);
    assert_file_contains(&test_path, test_name);
}

fn array_strings(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("expected string item in array, got: {item}"))
                .to_string()
        })
        .collect()
}

#[test]
fn public_surface_manifest_matches_inventories_and_fixtures() {
    let manifest_path = manifest_path();
    assert_path_exists(&manifest_path);

    let manifest = load_json(&manifest_path);
    assert_eq!(manifest["release"], "v0.47.10");
    assert_eq!(manifest["duckdb_version"], "1.5.3");
    assert_eq!(manifest["ducklake_version"], "1.0");
    assert_eq!(manifest["catalog_version"], 7);

    let surfaces = manifest["surfaces"]
        .as_array()
        .unwrap_or_else(|| panic!("surfaces must be an array"));
    assert!(!surfaces.is_empty(), "surface manifest must not be empty");

    let root = repo_root();
    let schema_fixture = root.join("tests/fixtures/ducklake-v1.0-schema.toml");
    let live_fixture =
        root.join("tests/fixtures/ducklake-corpus/duckdb-1.5.3-ducklake-1.0-live-surface.json");
    assert_path_exists(&schema_fixture);
    assert_path_exists(&live_fixture);

    let compatibility_snapshots = manifest["compatibility_snapshots"]
        .as_array()
        .unwrap_or_else(|| panic!("compatibility_snapshots must be an array"));
    assert!(
        compatibility_snapshots.len() >= 2,
        "compatibility_snapshots must include at least the previous and current release"
    );
    let mut snapshot_releases = Vec::new();
    for snapshot in compatibility_snapshots {
        let release = snapshot["release"]
            .as_str()
            .unwrap_or_else(|| panic!("snapshot must include release: {snapshot}"));
        snapshot_releases.push(release.to_string());
        assert!(
            snapshot["fixture_path"].as_str().is_some(),
            "snapshot must include fixture_path: {snapshot}"
        );
        if let Some(test_file) = snapshot["test_file"].as_str() {
            let test_name = snapshot["test_name"].as_str().unwrap_or_else(|| {
                panic!("compatibility snapshot must include test_name when test_file is set: {snapshot}")
            });
            assert_test_reference(&root, test_file, test_name);
        }
        assert_path_exists(
            &root.join(
                snapshot["fixture_path"]
                    .as_str()
                    .unwrap_or_else(|| panic!("snapshot must include fixture_path: {snapshot}")),
            ),
        );
    }
    assert!(
        snapshot_releases.iter().any(|release| release == "v0.47.10"),
        "compatibility snapshots must include the current release"
    );
    assert!(
        snapshot_releases.iter().any(|release| release == "v0.47.9"),
        "compatibility snapshots must include the previous release"
    );

    let negative_probes = manifest["negative_probes"]
        .as_array()
        .unwrap_or_else(|| panic!("negative_probes must be an array"));
    assert!(!negative_probes.is_empty(), "negative_probes must not be empty");
    for probe in negative_probes {
        let surface = probe["surface"]
            .as_str()
            .unwrap_or_else(|| panic!("negative probe must include surface: {probe}"));
        let test_file = probe["test_file"]
            .as_str()
            .unwrap_or_else(|| panic!("negative probe must include test_file: {probe}"));
        let test_name = probe["test_name"]
            .as_str()
            .unwrap_or_else(|| panic!("negative probe must include test_name: {probe}"));
        assert!(!surface.is_empty(), "negative probe surface must not be empty");
        assert_test_reference(&root, test_file, test_name);
    }

    let live_fixture_json = load_json(&live_fixture);

    let metadata_queries = live_fixture_json["metadata_queries"]
        .as_array()
        .unwrap_or_else(|| panic!("live fixture metadata_queries must be an array"));
    let metadata_sqls: Vec<String> = metadata_queries
        .iter()
        .map(|query| {
            query["sql"]
                .as_str()
                .unwrap_or_else(|| panic!("metadata query must include sql: {query}"))
                .to_string()
        })
        .collect();

    let mut covered_live_sqls = Vec::new();

    for surface in surfaces {
        let name = surface["name"]
            .as_str()
            .unwrap_or_else(|| panic!("surface must include name: {surface}"));
        let kind = surface["kind"]
            .as_str()
            .unwrap_or_else(|| panic!("surface must include kind: {surface}"));
        let status = surface["status"]
            .as_str()
            .unwrap_or_else(|| panic!("surface must include status: {surface}"));

        if let Some(test_file) = surface["test_file"].as_str() {
            let test_path = root.join(test_file);
            assert_path_exists(&test_path);
            if let Some(test_name) = surface["test_name"].as_str() {
                assert_file_contains(&test_path, test_name);
            }
        }

        if let Some(fixture_path) = surface["fixture_path"].as_str() {
            assert_path_exists(&root.join(fixture_path));
        }

        if status == "supported" {
            assert!(
                surface.get("test_file").is_some()
                    || surface.get("fixture_path").is_some()
                    || kind == "bindings",
                "supported surface {name:?} must have direct test or fixture evidence"
            );
        }

        match kind {
            "metadata_tables" => {
                let tables = surface["tables"].as_array().unwrap_or_else(|| {
                    panic!("metadata table surface must include tables: {surface}")
                });
                assert!(
                    !tables.is_empty(),
                    "metadata_tables surface must not be empty"
                );
                assert!(
                    tables.len() >= 20,
                    "metadata_tables surface should inventory the public DuckLake table set"
                );
            }
            "live_queries" => {
                let queries = surface["queries"].as_array().unwrap_or_else(|| {
                    panic!("live_queries surface must include queries: {surface}")
                });
                assert!(
                    !queries.is_empty(),
                    "live_queries surface must not be empty"
                );
                for query in queries {
                    let sql = query["sql"]
                        .as_str()
                        .unwrap_or_else(|| panic!("live query must include sql: {query}"));
                    covered_live_sqls.push(sql.to_string());
                    assert!(
                        metadata_sqls.iter().any(|candidate| candidate == sql),
                        "live fixture should include probe {sql:?}"
                    );
                    let expected_columns = query
                        .get("expected_columns")
                        .or_else(|| query.get("columns"))
                        .and_then(|value| value.as_array())
                        .map(|values| array_strings(values.as_slice()))
                        .unwrap_or_default();
                    let expected_types = query["expected_types"]
                        .as_array()
                        .map(|values| array_strings(values.as_slice()))
                        .unwrap_or_default();
                    if !expected_columns.is_empty() {
                        assert!(
                            !expected_types.is_empty(),
                            "expected_types missing for {sql}"
                        );
                        assert_eq!(
                            expected_columns.len(),
                            expected_types.len(),
                            "expected_columns and expected_types should have the same length for {sql}"
                        );
                    }
                }
            }
            "fixture_actions" => {
                let actions = surface["actions"].as_array().unwrap_or_else(|| {
                    panic!("fixture_actions surface must include actions: {surface}")
                });
                assert!(
                    !actions.is_empty(),
                    "fixture_actions surface must not be empty"
                );
                for action in actions {
                    let section = action["fixture_section"]
                        .as_str()
                        .unwrap_or_else(|| panic!("action must include fixture_section: {action}"));
                    let key = action["fixture_key"]
                        .as_str()
                        .unwrap_or_else(|| panic!("action must include fixture_key: {action}"));
                    let section_value = &live_fixture_json[section];
                    let actual = section_value[key].as_str().unwrap_or_else(|| {
                        panic!(
                            "fixture {section}.{key} must be a string in the live surface corpus"
                        )
                    });
                    assert!(
                        !actual.is_empty(),
                        "fixture {section}.{key} must not be empty"
                    );
                }
            }
            "sqlstate" => {
                let codes = surface["codes"]
                    .as_array()
                    .unwrap_or_else(|| panic!("sqlstate surface must include codes: {surface}"));
                assert!(!codes.is_empty(), "sqlstate surface must not be empty");
                for code in codes {
                    let code_value = code["code"]
                        .as_str()
                        .unwrap_or_else(|| panic!("code entry must include code: {code}"));
                    assert_eq!(code_value.len(), 5, "SQLSTATE codes must be 5 characters");
                }
            }
            "cli" => {
                let commands = surface["commands"]
                    .as_array()
                    .unwrap_or_else(|| panic!("cli surface must include commands: {surface}"));
                assert!(!commands.is_empty(), "cli surface must not be empty");
            }
            "bindings" => {
                let languages = surface["languages"].as_array().unwrap_or_else(|| {
                    panic!("bindings surface must include languages: {surface}")
                });
                assert!(!languages.is_empty(), "bindings surface must not be empty");
                for language in languages {
                    let test_file = language["test_file"].as_str().unwrap_or_else(|| {
                        panic!("binding entry must include test_file: {language}")
                    });
                    let test_name = language["test_name"].as_str().unwrap_or_else(|| {
                        panic!("binding entry must include test_name: {language}")
                    });
                    let negative_test_name = language["negative_test_name"].as_str().unwrap_or_else(|| {
                        panic!("binding entry must include negative_test_name: {language}")
                    });
                    assert_test_reference(&root, test_file, test_name);
                    assert_test_reference(&root, test_file, negative_test_name);
                }
            }
            "ffi" => {
                let functions = surface["functions"]
                    .as_array()
                    .unwrap_or_else(|| panic!("ffi surface must include functions: {surface}"));
                assert!(!functions.is_empty(), "ffi surface must not be empty");
            }
            "metrics" => {
                let probes = surface["probes"]
                    .as_array()
                    .unwrap_or_else(|| panic!("metrics surface must include probes: {surface}"));
                assert!(!probes.is_empty(), "metrics surface must not be empty");
            }
            "object_store_invariants" => {
                let invariants = surface["invariants"].as_array().unwrap_or_else(|| {
                    panic!("object_store_invariants surface must include invariants: {surface}")
                });
                assert!(
                    !invariants.is_empty(),
                    "object_store_invariants surface must not be empty"
                );
            }
            other => panic!("unexpected surface kind: {other}"),
        }
    }

    for sql in metadata_sqls {
        assert!(
            covered_live_sqls.iter().any(|candidate| candidate == &sql),
            "live fixture probe {sql:?} is missing from the manifest"
        );
    }
}
