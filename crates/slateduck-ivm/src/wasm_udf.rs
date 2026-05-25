//! WASM User-Defined Functions (UDFs) for IVM.
//!
//! Extends the view SQL surface with custom logic via WebAssembly:
//! deterministic, sandboxed, cross-platform. Compiled WASM modules are stored
//! as binary blobs in the catalog.
//!
//! ## Execution model
//! Per-batch pooled instance: a single `wasmtime::Instance` per UDF per batch,
//! reused across all rows. Memory limit (64 MiB) and fuel limit (10M instructions
//! × batch_size). Instance is dropped after batch completes.
//!
//! ## Determinism contract
//! All UDFs must be deterministic. Non-deterministic UDFs are rejected at
//! `CREATE FUNCTION` time. WASI imports are validated against a whitelist (none
//! for pure functions).

use serde::{Deserialize, Serialize};

/// UDF catalog entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdfEntry {
    /// Unique UDF identifier.
    pub udf_id: u64,
    /// Function name.
    pub name: String,
    /// Schema name.
    pub schema_name: String,
    /// WASM module binary.
    pub wasm_blob: Vec<u8>,
    /// Function signature.
    pub signature: UdfSignature,
    /// Must be true for IVM views.
    pub deterministic: bool,
    /// Snapshot when this UDF was created.
    pub created_at_snapshot: u64,
}

/// UDF function signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdfSignature {
    /// Argument types.
    pub arg_types: Vec<UdfType>,
    /// Return type.
    pub return_type: UdfType,
}

/// Supported UDF argument and return types (Arrow-compatible scalars).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UdfType {
    Boolean,
    Int8,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    Utf8,
    Binary,
    Date32,
    Timestamp,
}

impl std::fmt::Display for UdfType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UdfType::Boolean => write!(f, "BOOLEAN"),
            UdfType::Int8 => write!(f, "INT8"),
            UdfType::Int16 => write!(f, "INT16"),
            UdfType::Int32 => write!(f, "INT32"),
            UdfType::Int64 => write!(f, "INT64"),
            UdfType::Float32 => write!(f, "FLOAT32"),
            UdfType::Float64 => write!(f, "FLOAT64"),
            UdfType::Utf8 => write!(f, "UTF8"),
            UdfType::Binary => write!(f, "BINARY"),
            UdfType::Date32 => write!(f, "DATE32"),
            UdfType::Timestamp => write!(f, "TIMESTAMP"),
        }
    }
}

/// WASM execution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    /// Memory limit per instance in bytes (default: 64 MiB).
    pub memory_limit_bytes: u64,
    /// Fuel limit per row (default: 10_000_000 instructions).
    pub fuel_per_row: u64,
    /// Maximum batch fuel: fuel_per_row × batch_size.
    pub max_batch_fuel: u64,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            memory_limit_bytes: 64 * 1024 * 1024, // 64 MiB
            fuel_per_row: 10_000_000,
            max_batch_fuel: 10_000_000 * 10_000, // 10M × 10k rows
        }
    }
}

/// Result of validating a WASM module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmValidationResult {
    /// Module is valid for use in IVM.
    Valid,
    /// Module uses disallowed WASI imports.
    DisallowedImports(Vec<String>),
    /// Module is not deterministic.
    NotDeterministic,
    /// Module binary is invalid.
    InvalidModule(String),
}

/// Allowed WASI imports for pure functions (currently: none).
const ALLOWED_WASI_IMPORTS: &[&str] = &[];

/// Validate a WASM module for use in IVM UDFs.
pub fn validate_wasm_module(wasm_bytes: &[u8], deterministic: bool) -> WasmValidationResult {
    if !deterministic {
        return WasmValidationResult::NotDeterministic;
    }

    if wasm_bytes.is_empty() {
        return WasmValidationResult::InvalidModule("empty module".to_string());
    }

    // Check magic bytes (WASM magic: \0asm)
    if wasm_bytes.len() < 8 || &wasm_bytes[0..4] != b"\x00asm" {
        return WasmValidationResult::InvalidModule("invalid WASM magic bytes".to_string());
    }

    // Scan for WASI imports (simplified check)
    let disallowed = scan_wasi_imports(wasm_bytes);
    if !disallowed.is_empty() {
        return WasmValidationResult::DisallowedImports(disallowed);
    }

    WasmValidationResult::Valid
}

/// Scan WASM binary for WASI imports (simplified heuristic).
fn scan_wasi_imports(wasm_bytes: &[u8]) -> Vec<String> {
    let mut disallowed = Vec::new();

    // Look for known WASI function names in the binary
    let wasi_functions = [
        "fd_write",
        "fd_read",
        "fd_seek",
        "fd_close",
        "path_open",
        "environ_get",
        "environ_sizes_get",
        "proc_exit",
        "clock_time_get",
        "random_get",
    ];

    let bytes_str = String::from_utf8_lossy(wasm_bytes);
    for func in &wasi_functions {
        if bytes_str.contains(func) && !ALLOWED_WASI_IMPORTS.contains(func) {
            disallowed.push(func.to_string());
        }
    }

    disallowed
}

/// UDF execution errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UdfError {
    #[error("UDF '{name}' exceeded fuel limit ({fuel} instructions) on row {row_idx}")]
    FuelExhausted {
        name: String,
        fuel: u64,
        row_idx: u64,
    },
    #[error("UDF '{name}' exceeded memory limit ({limit_bytes} bytes)")]
    MemoryExhausted { name: String, limit_bytes: u64 },
    #[error("UDF '{name}' uses disallowed WASI import: {import}")]
    DisallowedImport { name: String, import: String },
    #[error("non-deterministic UDF not allowed in materialized views")]
    NotDeterministic,
    #[error("UDF module validation failed: {reason}")]
    ValidationFailed { reason: String },
    #[error("UDF '{name}' version {version} not found")]
    VersionNotFound { name: String, version: u64 },
}

/// UDF version migration request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdfVersionMigration {
    /// View that uses this UDF.
    pub view_name: String,
    /// UDF name.
    pub udf_name: String,
    /// Target UDF version (udf_id).
    pub target_version: u64,
    /// Whether to trigger REFRESH FULL after migration.
    pub trigger_full_refresh: bool,
}

/// UDF registry: in-memory index of available UDFs.
#[derive(Debug, Clone, Default)]
pub struct UdfRegistry {
    /// UDFs by name (latest version).
    pub entries: std::collections::HashMap<String, UdfEntry>,
    /// All versions by (name, udf_id).
    pub versions: std::collections::HashMap<(String, u64), UdfEntry>,
}

impl UdfRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new UDF.
    pub fn register(&mut self, entry: UdfEntry) -> Result<(), UdfError> {
        if !entry.deterministic {
            return Err(UdfError::NotDeterministic);
        }

        let validation = validate_wasm_module(&entry.wasm_blob, entry.deterministic);
        match validation {
            WasmValidationResult::Valid => {}
            WasmValidationResult::DisallowedImports(imports) => {
                return Err(UdfError::DisallowedImport {
                    name: entry.name.clone(),
                    import: imports.join(", "),
                });
            }
            WasmValidationResult::NotDeterministic => {
                return Err(UdfError::NotDeterministic);
            }
            WasmValidationResult::InvalidModule(reason) => {
                return Err(UdfError::ValidationFailed { reason });
            }
        }

        let key = (entry.name.clone(), entry.udf_id);
        self.versions.insert(key, entry.clone());
        self.entries.insert(entry.name.clone(), entry);
        Ok(())
    }

    /// Look up a UDF by name.
    pub fn get(&self, name: &str) -> Option<&UdfEntry> {
        self.entries.get(name)
    }

    /// Look up a specific version of a UDF.
    pub fn get_version(&self, name: &str, version: u64) -> Option<&UdfEntry> {
        self.versions.get(&(name.to_string(), version))
    }

    /// Drop a UDF by name.
    pub fn drop_function(&mut self, name: &str) -> bool {
        self.entries.remove(name).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_valid_wasm() -> Vec<u8> {
        // Minimal valid WASM module (magic + version + empty)
        let mut wasm = b"\x00asm".to_vec();
        wasm.extend_from_slice(&[1, 0, 0, 0]); // version 1
        wasm.extend_from_slice(&[0; 64]); // padding
        wasm
    }

    #[test]
    fn validate_valid_module() {
        let wasm = make_valid_wasm();
        assert_eq!(
            validate_wasm_module(&wasm, true),
            WasmValidationResult::Valid
        );
    }

    #[test]
    fn validate_rejects_non_deterministic() {
        let wasm = make_valid_wasm();
        assert_eq!(
            validate_wasm_module(&wasm, false),
            WasmValidationResult::NotDeterministic
        );
    }

    #[test]
    fn validate_rejects_empty_module() {
        assert!(matches!(
            validate_wasm_module(&[], true),
            WasmValidationResult::InvalidModule(_)
        ));
    }

    #[test]
    fn validate_rejects_disallowed_imports() {
        let mut wasm = make_valid_wasm();
        wasm.extend_from_slice(b"fd_write"); // Inject WASI import
        let result = validate_wasm_module(&wasm, true);
        assert!(matches!(result, WasmValidationResult::DisallowedImports(_)));
    }

    #[test]
    fn registry_register_and_lookup() {
        let mut registry = UdfRegistry::new();
        let entry = UdfEntry {
            udf_id: 1,
            name: "my_tokenizer".to_string(),
            schema_name: "public".to_string(),
            wasm_blob: make_valid_wasm(),
            signature: UdfSignature {
                arg_types: vec![UdfType::Utf8],
                return_type: UdfType::Utf8,
            },
            deterministic: true,
            created_at_snapshot: 1,
        };

        registry.register(entry.clone()).unwrap();
        assert!(registry.get("my_tokenizer").is_some());
        assert_eq!(registry.get("my_tokenizer").unwrap().udf_id, 1);
    }

    #[test]
    fn registry_rejects_non_deterministic() {
        let mut registry = UdfRegistry::new();
        let entry = UdfEntry {
            udf_id: 1,
            name: "bad_func".to_string(),
            schema_name: "public".to_string(),
            wasm_blob: make_valid_wasm(),
            signature: UdfSignature {
                arg_types: vec![],
                return_type: UdfType::Int64,
            },
            deterministic: false,
            created_at_snapshot: 1,
        };

        let result = registry.register(entry);
        assert!(matches!(result, Err(UdfError::NotDeterministic)));
    }

    #[test]
    fn registry_drop_function() {
        let mut registry = UdfRegistry::new();
        let entry = UdfEntry {
            udf_id: 1,
            name: "to_drop".to_string(),
            schema_name: "public".to_string(),
            wasm_blob: make_valid_wasm(),
            signature: UdfSignature {
                arg_types: vec![],
                return_type: UdfType::Boolean,
            },
            deterministic: true,
            created_at_snapshot: 1,
        };

        registry.register(entry).unwrap();
        assert!(registry.drop_function("to_drop"));
        assert!(registry.get("to_drop").is_none());
    }

    #[test]
    fn udf_type_display() {
        assert_eq!(UdfType::Boolean.to_string(), "BOOLEAN");
        assert_eq!(UdfType::Utf8.to_string(), "UTF8");
        assert_eq!(UdfType::Timestamp.to_string(), "TIMESTAMP");
    }
}
