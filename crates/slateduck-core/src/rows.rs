//! Row types for all 28 DuckLake tables.
//!
//! Each row type corresponds to a DuckLake catalog table and carries
//! all fields needed for serialization (Protobuf-compatible via serde).

use crate::mvcc::MvccFields;
use serde::{Deserialize, Serialize};

/// Metadata row (global and scoped key-value pairs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataRow {
    pub scope: u8,
    pub scope_id: u64,
    pub key: String,
    pub value: String,
}

/// Snapshot row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRow {
    pub snapshot_id: u64,
    pub schema_version: u64,
    pub created_at: String,
    pub author: Option<String>,
    pub message: Option<String>,
}

/// Snapshot changes row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotChangesRow {
    pub snapshot_id: u64,
    pub changes_json: String,
}

/// Schema row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaRow {
    pub schema_id: u64,
    pub name: String,
    pub mvcc: MvccFields,
}

/// Table row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    pub schema_id: u64,
    pub table_id: u64,
    pub name: String,
    pub uuid: String,
    pub mvcc: MvccFields,
}

/// Column row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnRow {
    pub table_id: u64,
    pub column_id: u64,
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    pub mvcc: MvccFields,
}

/// View row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewRow {
    pub schema_id: u64,
    pub view_id: u64,
    pub name: String,
    pub query: String,
    pub mvcc: MvccFields,
}

/// Macro row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroRow {
    pub schema_id: u64,
    pub macro_id: u64,
    pub name: String,
    pub macro_type: String,
    pub mvcc: MvccFields,
}

/// Macro implementation row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroImplRow {
    pub macro_id: u64,
    pub impl_id: u64,
    pub definition: String,
}

/// Macro parameters row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroParametersRow {
    pub macro_id: u64,
    pub impl_id: u64,
    pub column_id: u64,
    pub name: String,
    pub data_type: String,
    pub default_value: Option<String>,
}

/// Data file row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataFileRow {
    pub table_id: u64,
    pub data_file_id: u64,
    pub path: String,
    pub path_is_relative: bool,
    pub file_size_bytes: u64,
    pub record_count: u64,
    pub mvcc: MvccFields,
}

/// Delete file row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteFileRow {
    pub data_file_id: u64,
    pub delete_file_id: u64,
    pub path: String,
    pub path_is_relative: bool,
    pub file_size_bytes: u64,
    pub record_count: u64,
}

/// Files scheduled for deletion row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilesScheduledForDeletionRow {
    pub schedule_start: u64,
    pub data_file_id: u64,
    pub path: String,
}

/// Inlined data tables row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlinedDataTablesRow {
    pub table_id: u64,
    pub schema_version: u64,
    pub table_name: String,
}

/// Column mapping row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnMappingRow {
    pub table_id: u64,
    pub mapping_id: u64,
    pub mapping_json: String,
}

/// Name mapping row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameMappingRow {
    pub mapping_id: u64,
    pub column_id: u64,
    pub source_name_hash: u64,
    pub source_name: String,
    pub target_name: String,
}

/// Table statistics row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableStatsRow {
    pub table_id: u64,
    pub record_count: i64,
    pub file_count: u64,
    pub total_size_bytes: u64,
}

/// Table column statistics row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableColumnStatsRow {
    pub table_id: u64,
    pub column_id: u64,
    pub null_count: Option<u64>,
    pub distinct_count: Option<u64>,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
}

/// File column statistics row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileColumnStatsRow {
    pub table_id: u64,
    pub column_id: u64,
    pub data_file_id: u64,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub null_count: Option<u64>,
    pub contains_nan: bool,
}

/// File variant statistics row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileVariantStatsRow {
    pub table_id: u64,
    pub column_id: u64,
    pub variant_path_hash: u64,
    pub data_file_id: u64,
    pub variant_path: String,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
}

/// Partition info row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartitionInfoRow {
    pub table_id: u64,
    pub partition_id: u64,
    pub mvcc: MvccFields,
}

/// Partition column row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartitionColumnRow {
    pub partition_id: u64,
    pub partition_key_index: u64,
    pub column_id: u64,
    pub transform: String,
}

/// File partition value row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilePartitionValueRow {
    pub table_id: u64,
    pub partition_key_index: u64,
    pub data_file_id: u64,
    pub value: Option<String>,
}

/// Sort info row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortInfoRow {
    pub table_id: u64,
    pub sort_id: u64,
    pub mvcc: MvccFields,
}

/// Sort expression row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortExpressionRow {
    pub sort_id: u64,
    pub sort_key_index: u64,
    pub column_id: u64,
    pub ascending: bool,
    pub nulls_first: bool,
}

/// Tag row (object-level tags).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagRow {
    pub object_id: u64,
    pub tag_key: String,
    pub tag_value: String,
    pub mvcc: MvccFields,
}

/// Column tag row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnTagRow {
    pub table_id: u64,
    pub column_id: u64,
    pub tag_key: String,
    pub tag_value: String,
    pub mvcc: MvccFields,
}

/// Schema versions row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaVersionsRow {
    pub table_id: u64,
    pub begin_snapshot: u64,
    pub schema_version: u64,
}

/// Inlined insert row (under 0xFD subtype 0x01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlinedInsertRow {
    pub table_id: u64,
    pub schema_version: u64,
    pub row_id: u64,
    pub payload: Vec<u8>,
    pub begin_snapshot: u64,
    pub end_snapshot: Option<u64>,
}

/// Inlined delete marker (under 0xFD subtype 0x02).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlinedDeleteRow {
    pub table_id: u64,
    pub data_file_id: u64,
    pub row_id: u64,
    pub begin_snapshot: u64,
}

/// Enum of all possible catalog row types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CatalogRow {
    Metadata(MetadataRow),
    Snapshot(SnapshotRow),
    SnapshotChanges(SnapshotChangesRow),
    Schema(SchemaRow),
    Table(TableRow),
    Column(ColumnRow),
    View(ViewRow),
    Macro(MacroRow),
    MacroImpl(MacroImplRow),
    MacroParameters(MacroParametersRow),
    DataFile(DataFileRow),
    DeleteFile(DeleteFileRow),
    FilesScheduledForDeletion(FilesScheduledForDeletionRow),
    InlinedDataTables(InlinedDataTablesRow),
    ColumnMapping(ColumnMappingRow),
    NameMapping(NameMappingRow),
    TableStats(TableStatsRow),
    TableColumnStats(TableColumnStatsRow),
    FileColumnStats(FileColumnStatsRow),
    FileVariantStats(FileVariantStatsRow),
    PartitionInfo(PartitionInfoRow),
    PartitionColumn(PartitionColumnRow),
    FilePartitionValue(FilePartitionValueRow),
    SortInfo(SortInfoRow),
    SortExpression(SortExpressionRow),
    Tag(TagRow),
    ColumnTag(ColumnTagRow),
    SchemaVersions(SchemaVersionsRow),
    InlinedInsert(InlinedInsertRow),
    InlinedDelete(InlinedDeleteRow),
}
