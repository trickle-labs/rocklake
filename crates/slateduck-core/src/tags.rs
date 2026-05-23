//! Table tag bytes and key-layout definitions for all 28 DuckLake tables
//! plus SlateDuck system namespaces.
//!
//! This file is the single source of truth for the binary key layout.

/// Implementation status of a table in SlateDuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableStatus {
    /// Fully implemented and tested.
    Live,
    /// Deferred to a later phase.
    Deferred(u8),
    /// Not yet implemented.
    Unimplemented,
}

/// MVCC behavior for a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MvccBehavior {
    /// Row has begin_snapshot/end_snapshot columns; begin_snapshot in key.
    Versioned,
    /// Row is not versioned (e.g., statistics, metadata).
    Unversioned,
    /// System key with custom semantics.
    System,
}

/// Whether a unique-guard key is required for this table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniqueGuard {
    /// No guard needed — key layout naturally enforces uniqueness.
    None,
    /// Guard required under 0xFE prefix.
    Required,
}

/// Descriptor for a DuckLake table's key layout in SlateDB.
#[derive(Debug, Clone)]
pub struct TableDescriptor {
    pub tag: u8,
    pub name: &'static str,
    pub key_shape: &'static str,
    pub mvcc: MvccBehavior,
    pub unique_guard: UniqueGuard,
    pub status: TableStatus,
}

// -- Tag byte constants --

pub const TAG_DUCKLAKE_METADATA: u8 = 0x01;
pub const TAG_DUCKLAKE_SNAPSHOT: u8 = 0x02;
pub const TAG_DUCKLAKE_SNAPSHOT_CHANGES: u8 = 0x03;
pub const TAG_DUCKLAKE_SCHEMA: u8 = 0x04;
pub const TAG_DUCKLAKE_TABLE: u8 = 0x05;
pub const TAG_DUCKLAKE_COLUMN: u8 = 0x06;
pub const TAG_DUCKLAKE_VIEW: u8 = 0x07;
pub const TAG_DUCKLAKE_MACRO: u8 = 0x08;
pub const TAG_DUCKLAKE_MACRO_IMPL: u8 = 0x09;
pub const TAG_DUCKLAKE_MACRO_PARAMETERS: u8 = 0x0A;
pub const TAG_DUCKLAKE_DATA_FILE: u8 = 0x0B;
pub const TAG_DUCKLAKE_DELETE_FILE: u8 = 0x0C;
pub const TAG_DUCKLAKE_FILES_SCHEDULED_FOR_DELETION: u8 = 0x0D;
pub const TAG_DUCKLAKE_INLINED_DATA_TABLES: u8 = 0x0E;
pub const TAG_DUCKLAKE_COLUMN_MAPPING: u8 = 0x0F;
pub const TAG_DUCKLAKE_NAME_MAPPING: u8 = 0x10;
pub const TAG_DUCKLAKE_TABLE_STATS: u8 = 0x11;
pub const TAG_DUCKLAKE_TABLE_COLUMN_STATS: u8 = 0x12;
pub const TAG_DUCKLAKE_FILE_COLUMN_STATS: u8 = 0x13;
pub const TAG_DUCKLAKE_FILE_VARIANT_STATS: u8 = 0x14;
pub const TAG_DUCKLAKE_PARTITION_INFO: u8 = 0x15;
pub const TAG_DUCKLAKE_PARTITION_COLUMN: u8 = 0x16;
pub const TAG_DUCKLAKE_FILE_PARTITION_VALUE: u8 = 0x17;
pub const TAG_DUCKLAKE_SORT_INFO: u8 = 0x18;
pub const TAG_DUCKLAKE_SORT_EXPRESSION: u8 = 0x19;
pub const TAG_DUCKLAKE_TAG: u8 = 0x1A;
pub const TAG_DUCKLAKE_COLUMN_TAG: u8 = 0x1B;
pub const TAG_DUCKLAKE_SCHEMA_VERSIONS: u8 = 0x1C;
pub const TAG_DYNAMIC_INLINED_ROWS: u8 = 0xFD;
pub const TAG_SLATEDUCK_COUNTERS: u8 = 0xFE;
pub const TAG_SLATEDUCK_SYSTEM: u8 = 0xFF;

// -- Counter IDs under 0xFE --

pub const COUNTER_NEXT_SNAPSHOT_ID: u8 = 0x01;
pub const COUNTER_NEXT_CATALOG_ID: u8 = 0x02;
pub const COUNTER_NEXT_FILE_ID: u8 = 0x03;
pub const COUNTER_NEXT_COLUMN_ID_PREFIX: u8 = 0x10;

// -- Dynamic inlined row subtypes under 0xFD --

pub const INLINED_SUBTYPE_INSERT: u8 = 0x01;
pub const INLINED_SUBTYPE_DELETE: u8 = 0x02;

// -- System key identifiers under 0xFF --

pub const SYSTEM_WRITER_EPOCH: &[u8] = b"writer-epoch";
pub const SYSTEM_ENDPOINT: &[u8] = b"endpoint";
pub const SYSTEM_RETAIN_FROM: &[u8] = b"retain-from";
pub const SYSTEM_CATALOG_FORMAT_VERSION: &[u8] = b"catalog-format-version";

/// Current catalog format version.
pub const CATALOG_FORMAT_VERSION: u32 = 1;

/// All table descriptors in tag order.
pub static ALL_TABLES: &[TableDescriptor] = &[
    TableDescriptor {
        tag: TAG_DUCKLAKE_METADATA,
        name: "ducklake_metadata",
        key_shape: "scope | scope_id | metadata_key",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_SNAPSHOT,
        name: "ducklake_snapshot",
        key_shape: "snapshot_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_SNAPSHOT_CHANGES,
        name: "ducklake_snapshot_changes",
        key_shape: "snapshot_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_SCHEMA,
        name: "ducklake_schema",
        key_shape: "schema_id",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::Required,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_TABLE,
        name: "ducklake_table",
        key_shape: "schema_id | table_id | begin_snapshot",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::Required,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_COLUMN,
        name: "ducklake_column",
        key_shape: "table_id | column_id | begin_snapshot",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::Required,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_VIEW,
        name: "ducklake_view",
        key_shape: "schema_id | view_id | begin_snapshot",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::Required,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_MACRO,
        name: "ducklake_macro",
        key_shape: "schema_id | macro_id | begin_snapshot",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::Required,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_MACRO_IMPL,
        name: "ducklake_macro_impl",
        key_shape: "macro_id | impl_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_MACRO_PARAMETERS,
        name: "ducklake_macro_parameters",
        key_shape: "macro_id | impl_id | column_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_DATA_FILE,
        name: "ducklake_data_file",
        key_shape: "table_id | data_file_id",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_DELETE_FILE,
        name: "ducklake_delete_file",
        key_shape: "data_file_id | delete_file_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_FILES_SCHEDULED_FOR_DELETION,
        name: "ducklake_files_scheduled_for_deletion",
        key_shape: "schedule_start | data_file_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_INLINED_DATA_TABLES,
        name: "ducklake_inlined_data_tables",
        key_shape: "table_id | schema_version",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_COLUMN_MAPPING,
        name: "ducklake_column_mapping",
        key_shape: "table_id | mapping_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_NAME_MAPPING,
        name: "ducklake_name_mapping",
        key_shape: "mapping_id | column_id | source_name_hash",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_TABLE_STATS,
        name: "ducklake_table_stats",
        key_shape: "table_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_TABLE_COLUMN_STATS,
        name: "ducklake_table_column_stats",
        key_shape: "table_id | column_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_FILE_COLUMN_STATS,
        name: "ducklake_file_column_stats",
        key_shape: "table_id | column_id | data_file_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_FILE_VARIANT_STATS,
        name: "ducklake_file_variant_stats",
        key_shape: "table_id | column_id | variant_path_hash | data_file_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_PARTITION_INFO,
        name: "ducklake_partition_info",
        key_shape: "table_id | partition_id | begin_snapshot",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_PARTITION_COLUMN,
        name: "ducklake_partition_column",
        key_shape: "partition_id | partition_key_index",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_FILE_PARTITION_VALUE,
        name: "ducklake_file_partition_value",
        key_shape: "table_id | partition_key_index | data_file_id",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_SORT_INFO,
        name: "ducklake_sort_info",
        key_shape: "table_id | sort_id | begin_snapshot",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_SORT_EXPRESSION,
        name: "ducklake_sort_expression",
        key_shape: "sort_id | sort_key_index",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_TAG,
        name: "ducklake_tag",
        key_shape: "object_id | tag_key | begin_snapshot",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_COLUMN_TAG,
        name: "ducklake_column_tag",
        key_shape: "table_id | column_id | tag_key | begin_snapshot",
        mvcc: MvccBehavior::Versioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
    TableDescriptor {
        tag: TAG_DUCKLAKE_SCHEMA_VERSIONS,
        name: "ducklake_schema_versions",
        key_shape: "table_id | begin_snapshot",
        mvcc: MvccBehavior::Unversioned,
        unique_guard: UniqueGuard::None,
        status: TableStatus::Live,
    },
];

/// Look up a table descriptor by tag byte.
pub fn table_by_tag(tag: u8) -> Option<&'static TableDescriptor> {
    ALL_TABLES.iter().find(|t| t.tag == tag)
}

/// Verify all tags are unique and in ascending order.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_unique_and_ordered() {
        let mut prev = 0u8;
        for (i, table) in ALL_TABLES.iter().enumerate() {
            if i > 0 {
                assert!(
                    table.tag > prev,
                    "Tag 0x{:02X} for {} is not greater than previous 0x{:02X}",
                    table.tag,
                    table.name,
                    prev
                );
            }
            prev = table.tag;
        }
    }

    #[test]
    fn all_28_tables_present() {
        assert_eq!(ALL_TABLES.len(), 28);
    }

    #[test]
    fn no_tag_collisions_with_system_ranges() {
        for table in ALL_TABLES {
            assert!(
                table.tag < TAG_DYNAMIC_INLINED_ROWS,
                "Table {} has tag 0x{:02X} which collides with system range",
                table.name,
                table.tag
            );
        }
    }
}
