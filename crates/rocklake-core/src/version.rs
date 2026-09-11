//! Version domains shared by persisted RockLake contracts.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The DuckLake catalog protocol version stored in snapshot metadata.
pub const DUCKLAKE_CATALOG_VERSION: u64 = 7;
/// The RockLake catalog storage/key/value format version.
pub const CATALOG_STORAGE_VERSION: u32 = 1;
/// The managed registry format version.
pub const REGISTRY_VERSION: u32 = 1;
/// The catalog backup manifest format version.
pub const BACKUP_MANIFEST_VERSION: u32 = 2;
/// The managed service backup-set manifest format version.
pub const BACKUP_SET_MANIFEST_VERSION: u32 = 1;
/// The administrative job ledger format version.
pub const JOB_LEDGER_VERSION: u32 = 1;
/// The catalog audit-chain schema version.
pub const AUDIT_SCHEMA_VERSION: u32 = 1;
/// The public JSON output schema version.
pub const PUBLIC_JSON_SCHEMA_VERSION: u32 = 1;
/// The evidence result schema version.
pub const EVIDENCE_SCHEMA_VERSION: u32 = 1;

macro_rules! version_type {
    ($name:ident, $inner:ty, $label:literal) => {
        /// A version in one persisted contract domain.
        #[derive(
            Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);

        impl $name {
            /// Construct a version value.
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }

            /// Return the numeric version.
            pub const fn get(self) -> $inner {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{} v{}", $label, self.0)
            }
        }
    };
}

version_type!(DuckLakeCatalogVersion, u64, "DuckLake catalog");
version_type!(CatalogStorageVersion, u32, "RockLake catalog storage");
version_type!(RegistryVersion, u32, "registry");
version_type!(BackupManifestVersion, u32, "backup manifest");
version_type!(JobLedgerVersion, u32, "job ledger");
version_type!(AuditSchemaVersion, u32, "audit schema");
version_type!(PublicJsonSchemaVersion, u32, "public JSON");
version_type!(EvidenceSchemaVersion, u32, "evidence schema");

/// All persisted compatibility domains reported by `rocklake status`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionSet {
    /// DuckLake catalog protocol/schema version.
    pub ducklake_catalog: DuckLakeCatalogVersion,
    /// RockLake catalog storage/key/value format.
    pub catalog_storage: CatalogStorageVersion,
    /// Managed registry format.
    pub registry: RegistryVersion,
    /// Catalog backup manifest format.
    pub backup_manifest: BackupManifestVersion,
    /// Administrative job ledger format.
    pub job_ledger: JobLedgerVersion,
    /// Catalog audit-chain schema.
    pub audit_schema: AuditSchemaVersion,
    /// Public JSON output schema.
    pub public_json: PublicJsonSchemaVersion,
    /// Evidence result schema.
    pub evidence_schema: EvidenceSchemaVersion,
}

impl VersionSet {
    /// Return the versions supported by this binary.
    pub const fn current() -> Self {
        Self {
            ducklake_catalog: DuckLakeCatalogVersion(DUCKLAKE_CATALOG_VERSION),
            catalog_storage: CatalogStorageVersion(CATALOG_STORAGE_VERSION),
            registry: RegistryVersion(REGISTRY_VERSION),
            backup_manifest: BackupManifestVersion(BACKUP_MANIFEST_VERSION),
            job_ledger: JobLedgerVersion(JOB_LEDGER_VERSION),
            audit_schema: AuditSchemaVersion(AUDIT_SCHEMA_VERSION),
            public_json: PublicJsonSchemaVersion(PUBLIC_JSON_SCHEMA_VERSION),
            evidence_schema: EvidenceSchemaVersion(EVIDENCE_SCHEMA_VERSION),
        }
    }
}

/// The current version contract used by the binary and persisted formats.
pub const CURRENT_VERSIONS: VersionSet = VersionSet::current();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_versions_are_machine_readable() {
        let json = serde_json::to_value(CURRENT_VERSIONS).unwrap();
        assert_eq!(json["ducklake_catalog"], 7);
        assert_eq!(json["catalog_storage"], 1);
        assert_eq!(json["backup_manifest"], 2);
    }

    #[test]
    fn version_display_names_domain() {
        assert_eq!(
            CatalogStorageVersion::new(1).to_string(),
            "RockLake catalog storage v1"
        );
    }
}
