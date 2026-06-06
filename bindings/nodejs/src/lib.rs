//! Node.js (napi-rs) bindings for the RockLake catalog.
//!
//! Exposes `Catalog`, `Snapshot`, `Schema`, `Table`, and `DataFile`
//! classes with both callback and Promise-based async APIs.
//!
//! # ID representation (v0.46.0)
//!
//! All numeric ID fields (`snapshot_id`, `schema_id`, `table_id`,
//! `data_file_id`, `row_count`, `file_size_bytes`) are exposed as JavaScript
//! `BigInt` to avoid truncation of IDs above `u32::MAX` (~4 billion).
//!
//! In JavaScript, access them as:
//! ```js
//! const snap = cat.snapshotId(); // BigInt
//! const n = Number(snap);        // safe only if value < 2^53
//! ```

#![deny(clippy::all)]

use std::convert::TryFrom;

use napi::{Env, JsBigInt};
use napi_derive::napi;

use rocklake_client::CatalogClientSync;

// ─── Value types ───────────────────────────────────────────────────────────

/// Snapshot metadata.
#[napi(object)]
pub struct Snapshot {
    /// Current snapshot ID (0 = empty catalog).
    pub snapshot_id: JsBigInt,
}

/// A catalog schema.
#[napi(object)]
pub struct Schema {
    pub schema_id: JsBigInt,
    pub schema_name: String,
}

/// A catalog table.
#[napi(object)]
pub struct Table {
    pub table_id: JsBigInt,
    pub schema_id: JsBigInt,
    pub table_name: String,
}

/// A data file registered in the catalog.
#[napi(object)]
pub struct DataFile {
    pub data_file_id: JsBigInt,
    pub table_id: JsBigInt,
    pub path: String,
    pub file_format: String,
    pub row_count: JsBigInt,
    pub file_size_bytes: JsBigInt,
    pub snapshot_id: JsBigInt,
}

// ─── Catalog class ─────────────────────────────────────────────────────────

/// Open RockLake catalog.
///
/// ```js
/// const { Catalog } = require('@rocklake/client');
///
/// const cat = Catalog.open('/path/to/catalog');
/// const snap = cat.snapshotId();
/// const schemas = cat.listSchemas(snap);
/// cat.close();
/// ```
#[napi]
pub struct Catalog {
    inner: Option<CatalogClientSync>,
}

#[napi]
impl Catalog {
    /// Open a catalog at *uri*.
    #[napi(factory)]
    pub fn open(uri: String) -> napi::Result<Self> {
        let inner =
            CatalogClientSync::open(&uri).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(Self { inner: Some(inner) })
    }

    /// Open a catalog in read-only mode (no writer epoch acquired).
    ///
    /// Use this for stateless reader replicas and analytics sidecars. Multiple
    /// simultaneous calls produce zero CAS write conflicts.
    #[napi(factory)]
    pub fn open_readonly(uri: String) -> napi::Result<Self> {
        let inner = CatalogClientSync::open_readonly(&uri)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(Self { inner: Some(inner) })
    }

    /// Return the current snapshot ID as a `BigInt`.
    #[napi]
    pub fn snapshot_id(&self, env: Env) -> napi::Result<JsBigInt> {
        let id = self
            .client()?
            .snapshot_id()
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        env.create_bigint_from_u64(id)
    }

    /// Return the current snapshot.
    #[napi]
    pub fn current_snapshot(&self, env: Env) -> napi::Result<Snapshot> {
        Ok(Snapshot {
            snapshot_id: self.snapshot_id(env)?,
        })
    }

    /// List schemas at *snapshotId* (0 = latest).
    #[napi]
    pub fn list_schemas(&self, env: Env, snapshot_id: JsBigInt) -> napi::Result<Vec<Schema>> {
        let snapshot_id = u64::try_from(snapshot_id).map_err(|_| {
            napi::Error::from_reason("snapshot_id must be a non-negative, lossless BigInt")
        })?;
        let schemas = self
            .client()?
            .list_schemas(snapshot_id)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(schemas
            .into_iter()
            .map(|s| Schema {
                schema_id: env.create_bigint_from_u64(s.schema_id).unwrap(),
                schema_name: s.schema_name,
            })
            .collect())
    }

    /// List tables in *schemaId* at *snapshotId*.
    #[napi]
    pub fn list_tables(&self, env: Env, schema_id: JsBigInt, snapshot_id: JsBigInt) -> napi::Result<Vec<Table>> {
        let schema_id = u64::try_from(schema_id).map_err(|_| {
            napi::Error::from_reason("schema_id must be a non-negative, lossless BigInt")
        })?;
        let snapshot_id = u64::try_from(snapshot_id).map_err(|_| {
            napi::Error::from_reason("snapshot_id must be a non-negative, lossless BigInt")
        })?;
        let tables = self
            .client()?
            .list_tables(schema_id, snapshot_id)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(tables
            .into_iter()
            .map(|t| Table {
                table_id: env.create_bigint_from_u64(t.table_id).unwrap(),
                schema_id: env.create_bigint_from_u64(t.schema_id).unwrap(),
                table_name: t.table_name,
            })
            .collect())
    }

    /// List data files for *tableId* at *snapshotId*.
    #[napi]
    pub fn list_data_files(&self, env: Env, table_id: JsBigInt, snapshot_id: JsBigInt) -> napi::Result<Vec<DataFile>> {
        let table_id = u64::try_from(table_id).map_err(|_| {
            napi::Error::from_reason("table_id must be a non-negative, lossless BigInt")
        })?;
        let snapshot_id = u64::try_from(snapshot_id).map_err(|_| {
            napi::Error::from_reason("snapshot_id must be a non-negative, lossless BigInt")
        })?;
        let files = self
            .client()?
            .list_data_files(table_id, snapshot_id)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(files
            .into_iter()
            .map(|f| DataFile {
                data_file_id: env.create_bigint_from_u64(f.data_file_id).unwrap(),
                table_id: env.create_bigint_from_u64(f.table_id).unwrap(),
                path: f.path,
                file_format: f.file_format,
                row_count: env.create_bigint_from_u64(f.row_count).unwrap(),
                file_size_bytes: env.create_bigint_from_u64(f.file_size_bytes).unwrap(),
                snapshot_id: env.create_bigint_from_u64(f.snapshot_id).unwrap(),
            })
            .collect())
    }

    /// Close the catalog.
    #[napi]
    pub fn close(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.close();
        }
    }
}

impl Catalog {
    fn client(&self) -> napi::Result<&CatalogClientSync> {
        self.inner.as_ref().ok_or_else(|| {
            napi::Error::from_reason("catalog has been closed".to_string())
        })
    }
}
