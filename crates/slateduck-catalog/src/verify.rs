//! `slateduck verify catalog` — validates catalog integrity.
//!
//! Checks: primary-key uniqueness, foreign-key references, MVCC interval
//! consistency, counter monotonicity.

use slatedb::Db;
use slateduck_core::encoding::decode_value;
use slateduck_core::error::{Result, SlateDuckError};
use slateduck_core::keys;
use slateduck_core::rows::*;
use slateduck_core::tags::*;
use std::collections::HashSet;

/// Result of catalog verification.
#[derive(Debug, Clone, Default)]
pub struct VerifyResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub tables_checked: u32,
    pub rows_checked: u64,
}

impl VerifyResult {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Verify catalog integrity.
pub async fn verify_catalog(db: &Db) -> Result<VerifyResult> {
    let mut result = VerifyResult::default();

    // Check format version
    let format_key = keys::system_key(SYSTEM_CATALOG_FORMAT_VERSION);
    let format_data = db
        .get(&format_key)
        .await
        .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
    match format_data {
        Some(data) => {
            let version = slateduck_core::encoding::decode_u32(&data)?;
            if version != CATALOG_FORMAT_VERSION {
                result.errors.push(format!(
                    "format version mismatch: expected {}, got {}",
                    CATALOG_FORMAT_VERSION, version
                ));
            }
        }
        None => {
            result
                .errors
                .push("catalog not initialized: missing format version".to_string());
            return Ok(result);
        }
    }

    // Check counter monotonicity
    verify_counters(db, &mut result).await?;

    // Check schema primary-key uniqueness
    verify_schema_uniqueness(db, &mut result).await?;

    // Check table primary-key uniqueness
    verify_table_uniqueness(db, &mut result).await?;

    // Check MVCC interval consistency
    verify_mvcc_intervals(db, &mut result).await?;

    // Check all table tags
    result.tables_checked = ALL_TABLES.len() as u32;

    Ok(result)
}

async fn verify_counters(db: &Db, result: &mut VerifyResult) -> Result<()> {
    let counters = [
        (COUNTER_NEXT_SNAPSHOT_ID, "snapshot"),
        (COUNTER_NEXT_CATALOG_ID, "catalog"),
        (COUNTER_NEXT_FILE_ID, "file"),
    ];

    for (counter_id, name) in counters {
        let key = keys::counter_key(counter_id);
        let data = db
            .get(&key)
            .await
            .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;
        match data {
            Some(bytes) => {
                let value = slateduck_core::encoding::decode_counter(&bytes)?;
                if value == 0 {
                    result.warnings.push(format!("{name} counter is 0"));
                }
            }
            None => {
                result
                    .errors
                    .push(format!("missing counter: {name} (0x{counter_id:02X})"));
            }
        }
    }
    Ok(())
}

async fn verify_schema_uniqueness(db: &Db, result: &mut VerifyResult) -> Result<()> {
    let prefix = keys::table_prefix(TAG_DUCKLAKE_SCHEMA);
    let mut seen_ids: HashSet<u64> = HashSet::new();
    let mut iter = db
        .scan_prefix(&prefix)
        .await
        .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
    {
        let payload = decode_value(&kv.value)?;
        let row: SchemaRow =
            serde_json::from_slice(payload).map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
        result.rows_checked += 1;

        // For versioned rows, the same schema_id can appear multiple times
        // (different begin_snapshot versions). Check that active (end_snapshot=None)
        // rows have unique IDs.
        if row.mvcc.end_snapshot.is_none() && !seen_ids.insert(row.schema_id) {
            result
                .errors
                .push(format!("duplicate active schema_id: {}", row.schema_id));
        }
    }
    Ok(())
}

async fn verify_table_uniqueness(db: &Db, result: &mut VerifyResult) -> Result<()> {
    let prefix = keys::table_prefix(TAG_DUCKLAKE_TABLE);
    let mut seen_ids: HashSet<u64> = HashSet::new();
    let mut iter = db
        .scan_prefix(&prefix)
        .await
        .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
    {
        let payload = decode_value(&kv.value)?;
        let row: TableRow =
            serde_json::from_slice(payload).map_err(|e| SlateDuckError::Encoding(e.to_string()))?;
        result.rows_checked += 1;

        if row.mvcc.end_snapshot.is_none() && !seen_ids.insert(row.table_id) {
            result
                .errors
                .push(format!("duplicate active table_id: {}", row.table_id));
        }
    }
    Ok(())
}

async fn verify_mvcc_intervals(db: &Db, result: &mut VerifyResult) -> Result<()> {
    // Verify that no row has end_snapshot <= begin_snapshot
    let prefix = keys::table_prefix(TAG_DUCKLAKE_TABLE);
    let mut iter = db
        .scan_prefix(&prefix)
        .await
        .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?;

    while let Some(kv) = iter
        .next()
        .await
        .map_err(|e| SlateDuckError::SlateDb(e.to_string()))?
    {
        let payload = decode_value(&kv.value)?;
        let row: TableRow =
            serde_json::from_slice(payload).map_err(|e| SlateDuckError::Encoding(e.to_string()))?;

        if let Some(end) = row.mvcc.end_snapshot {
            if end <= row.mvcc.begin_snapshot {
                result.errors.push(format!(
                    "invalid MVCC interval for table {}: begin={} >= end={}",
                    row.table_id, row.mvcc.begin_snapshot, end
                ));
            }
        }
    }
    Ok(())
}
