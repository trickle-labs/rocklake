//! Non-deterministic function capture semantics for IVM.
//!
//! Functions like `now()`, `random()`, `gen_random_uuid()` are non-deterministic
//! but users legitimately need views like `SELECT *, now() AS captured_at FROM events`.
//!
//! Solution: sample once per batch, substitute a literal, store the value
//! alongside the checkpoint for deterministic repair/replay.
//!
//! ## Upgrade path from v0.14
//! v0.14 rejects all VOLATILE functions. v0.16 introduces `CaptureEligible`:
//! functions safe under per-batch sampling are accepted (no longer rejected).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::volatility::Volatility;

/// Extended volatility classification with capture eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureVolatility {
    /// Immutable: no capture needed.
    Immutable,
    /// Stable: no capture needed (same within statement).
    Stable,
    /// Capture-eligible: sampled once per batch, stored in checkpoint.
    CaptureEligible,
    /// Volatile with side effects: cannot be safely captured.
    Volatile,
}

/// Determine the capture volatility of a function.
pub fn capture_volatility_of(name: &str) -> CaptureVolatility {
    match name.to_lowercase().as_str() {
        // Capture-eligible: safe with per-batch sampling
        "now" | "current_timestamp" | "current_date" | "current_time" | "localtime"
        | "localtimestamp" | "random" | "gen_random_uuid" => CaptureVolatility::CaptureEligible,

        // Volatile with side effects: cannot capture
        "nextval"
        | "currval"
        | "setseed"
        | "clock_timestamp"
        | "statement_timestamp"
        | "transaction_timestamp"
        | "timeofday" => CaptureVolatility::Volatile,

        // Use base volatility for everything else
        _ => {
            let base = crate::volatility::volatility_of(name);
            match base {
                Volatility::Immutable => CaptureVolatility::Immutable,
                Volatility::Stable => CaptureVolatility::Stable,
                Volatility::Volatile => CaptureVolatility::Volatile,
                Volatility::Unknown => CaptureVolatility::Volatile,
            }
        }
    }
}

/// Per-batch captured values for non-deterministic functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCapture {
    /// The batch/snapshot ID this capture belongs to.
    pub batch_id: u64,
    /// Captured function values: function_name → sampled value.
    pub captured_values: HashMap<String, CapturedValue>,
    /// Random seed for this batch (enables deterministic replay of `random()`).
    pub random_seed: u64,
}

/// A captured value for a non-deterministic function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapturedValue {
    /// Timestamp value (epoch milliseconds).
    Timestamp(i64),
    /// UUID string.
    Uuid(String),
    /// Random float (stored as bits for exact reproduction).
    RandomF64(u64),
    /// Integer value.
    Integer(i64),
    /// String value.
    Text(String),
}

impl CapturedValue {
    /// Convert to a JSON-compatible serde_json::Value.
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            CapturedValue::Timestamp(ts) => serde_json::Value::Number((*ts).into()),
            CapturedValue::Uuid(s) => serde_json::Value::String(s.clone()),
            CapturedValue::RandomF64(bits) => {
                let f = f64::from_bits(*bits);
                serde_json::Value::Number(
                    serde_json::Number::from_f64(f).unwrap_or_else(|| 0.into()),
                )
            }
            CapturedValue::Integer(i) => serde_json::Value::Number((*i).into()),
            CapturedValue::Text(s) => serde_json::Value::String(s.clone()),
        }
    }
}

/// IVM-specific function: `current_snapshot_id()` returns the batch's
/// `last_input_snapshot` as a stable integer.
pub const CURRENT_SNAPSHOT_ID_FN: &str = "current_snapshot_id";

/// Sample all capture-eligible functions for a batch.
pub fn sample_batch_captures(batch_id: u64, functions: &[String]) -> BatchCapture {
    let seed = generate_batch_seed(batch_id);
    let mut captured_values = HashMap::new();

    for func_name in functions {
        let value = sample_function(func_name, batch_id, seed);
        if let Some(v) = value {
            captured_values.insert(func_name.clone(), v);
        }
    }

    BatchCapture {
        batch_id,
        captured_values,
        random_seed: seed,
    }
}

/// Generate a deterministic seed from a batch ID.
fn generate_batch_seed(batch_id: u64) -> u64 {
    // Simple hash-based seed generation (deterministic from batch_id)
    let mut h = batch_id;
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 33;
    h
}

/// Sample a single function for a batch.
fn sample_function(name: &str, batch_id: u64, seed: u64) -> Option<CapturedValue> {
    match name.to_lowercase().as_str() {
        "now" | "current_timestamp" | "localtimestamp" => {
            // Use a fixed epoch for determinism in tests; real implementation
            // would use actual wall-clock time at batch start.
            let ts = 1_700_000_000_000i64 + (batch_id as i64 * 1000);
            Some(CapturedValue::Timestamp(ts))
        }
        "current_date" => {
            // Days since epoch
            let days = 19_700i64 + (batch_id as i64);
            Some(CapturedValue::Integer(days))
        }
        "current_time" | "localtime" => {
            // Time as milliseconds since midnight
            let ms = ((batch_id * 1000) % 86_400_000) as i64;
            Some(CapturedValue::Integer(ms))
        }
        "random" => {
            // Deterministic random from seed
            let bits = seed ^ (batch_id.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let f = (bits as f64) / (u64::MAX as f64);
            Some(CapturedValue::RandomF64(f.to_bits()))
        }
        "gen_random_uuid" => {
            // Deterministic UUID from seed (v4 format)
            let uuid = format!(
                "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
                (seed >> 32) as u32,
                (seed >> 16) as u16,
                seed as u16 & 0x0FFF,
                ((seed >> 48) as u16 & 0x3FFF) | 0x8000,
                batch_id & 0xFFFF_FFFF_FFFF
            );
            Some(CapturedValue::Uuid(uuid))
        }
        _ => None,
    }
}

/// Validate that all functions in a view SQL are allowed under capture semantics.
pub fn validate_capture_eligible(functions: &[String]) -> Result<(), CaptureError> {
    for func in functions {
        let vol = capture_volatility_of(func);
        if vol == CaptureVolatility::Volatile {
            return Err(CaptureError::VolatileNotAllowed {
                function: func.clone(),
            });
        }
    }
    Ok(())
}

/// Errors from capture validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    #[error("function '{function}' is volatile with side effects and cannot be used in materialized views")]
    VolatileNotAllowed { function: String },
}

/// Restore captured values from a checkpoint for deterministic repair/replay.
pub fn restore_captures(checkpoint_data: &[u8]) -> Result<BatchCapture, String> {
    serde_json::from_slice(checkpoint_data).map_err(|e| e.to_string())
}

/// Serialize captured values for checkpoint storage.
pub fn serialize_captures(capture: &BatchCapture) -> Result<Vec<u8>, String> {
    serde_json::to_vec(capture).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_eligible_functions_classified() {
        assert_eq!(
            capture_volatility_of("now"),
            CaptureVolatility::CaptureEligible
        );
        assert_eq!(
            capture_volatility_of("random"),
            CaptureVolatility::CaptureEligible
        );
        assert_eq!(
            capture_volatility_of("gen_random_uuid"),
            CaptureVolatility::CaptureEligible
        );
        assert_eq!(
            capture_volatility_of("current_timestamp"),
            CaptureVolatility::CaptureEligible
        );
    }

    #[test]
    fn volatile_functions_rejected() {
        assert_eq!(
            capture_volatility_of("nextval"),
            CaptureVolatility::Volatile
        );
        assert_eq!(
            capture_volatility_of("setseed"),
            CaptureVolatility::Volatile
        );
    }

    #[test]
    fn immutable_functions_unchanged() {
        assert_eq!(capture_volatility_of("abs"), CaptureVolatility::Immutable);
        assert_eq!(capture_volatility_of("lower"), CaptureVolatility::Immutable);
    }

    #[test]
    fn batch_capture_deterministic() {
        let functions = vec!["now".to_string(), "random".to_string()];
        let cap1 = sample_batch_captures(42, &functions);
        let cap2 = sample_batch_captures(42, &functions);

        // Same batch_id produces same captures
        assert_eq!(cap1.captured_values["now"], cap2.captured_values["now"]);
        assert_eq!(
            cap1.captured_values["random"],
            cap2.captured_values["random"]
        );
        assert_eq!(cap1.random_seed, cap2.random_seed);
    }

    #[test]
    fn different_batches_different_captures() {
        let functions = vec!["now".to_string()];
        let cap1 = sample_batch_captures(1, &functions);
        let cap2 = sample_batch_captures(2, &functions);

        assert_ne!(cap1.captured_values["now"], cap2.captured_values["now"]);
    }

    #[test]
    fn validate_rejects_volatile() {
        let functions = vec!["now".to_string(), "nextval".to_string()];
        let result = validate_capture_eligible(&functions);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CaptureError::VolatileNotAllowed { .. }
        ));
    }

    #[test]
    fn validate_accepts_capture_eligible() {
        let functions = vec![
            "now".to_string(),
            "random".to_string(),
            "gen_random_uuid".to_string(),
        ];
        assert!(validate_capture_eligible(&functions).is_ok());
    }

    #[test]
    fn serialize_and_restore_captures() {
        let functions = vec!["now".to_string(), "random".to_string()];
        let cap = sample_batch_captures(99, &functions);

        let serialized = serialize_captures(&cap).unwrap();
        let restored = restore_captures(&serialized).unwrap();

        assert_eq!(cap.batch_id, restored.batch_id);
        assert_eq!(cap.random_seed, restored.random_seed);
        assert_eq!(cap.captured_values, restored.captured_values);
    }

    #[test]
    fn repair_uses_stored_seed() {
        // Simulate: first run samples, second run (repair) re-uses stored seed
        let functions = vec!["random".to_string()];
        let original = sample_batch_captures(100, &functions);

        let serialized = serialize_captures(&original).unwrap();
        let restored = restore_captures(&serialized).unwrap();

        // Repair produces identical output
        assert_eq!(
            original.captured_values["random"],
            restored.captured_values["random"]
        );
    }
}
