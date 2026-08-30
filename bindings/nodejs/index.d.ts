// TypeScript declarations for @rocklake/client
//
// All numeric ID fields are `bigint` to avoid truncation of
// values above `u32::MAX` (~4 billion).  Use `Number(id)` only when you
// are certain the value fits in a safe integer (< 2^53).

export interface Snapshot {
  snapshotId: bigint;
}

export interface Schema {
  schemaId: bigint;
  schemaName: string;
}

export interface Table {
  tableId: bigint;
  schemaId: bigint;
  tableName: string;
}

export interface DataFile {
  dataFileId: bigint;
  tableId: bigint;
  path: string;
  fileFormat: string;
  rowCount: bigint;
  fileSizeBytes: bigint;
  snapshotId: bigint;
}

export declare class Catalog {
  /** Open a catalog at *uri*. */
  static open(uri: string): Catalog;
  /** Return the current snapshot ID (0n = empty catalog). */
  snapshotId(): bigint;
  /** Return the current snapshot. */
  currentSnapshot(): Snapshot;
  /** List schemas at an exact snapshot ID. */
  listSchemas(snapshotId: bigint): Schema[];
  /** List schemas at the latest committed snapshot. */
  listSchemasLatest(): Schema[];
  /** List schemas at an exact snapshot ID. */
  listSchemasAt(snapshotId: bigint): Schema[];
  /** List tables in *schemaId* at an exact snapshot ID. */
  listTables(schemaId: bigint, snapshotId: bigint): Table[];
  /** List tables in *schemaId* at the latest committed snapshot. */
  listTablesLatest(schemaId: bigint): Table[];
  /** List tables in *schemaId* at an exact snapshot ID. */
  listTablesAt(schemaId: bigint, snapshotId: bigint): Table[];
  /** List data files for *tableId* at an exact snapshot ID. */
  listDataFiles(tableId: bigint, snapshotId: bigint): DataFile[];
  /** List data files for *tableId* at the latest committed snapshot. */
  listDataFilesLatest(tableId: bigint): DataFile[];
  /** List data files for *tableId* at an exact snapshot ID. */
  listDataFilesAt(tableId: bigint, snapshotId: bigint): DataFile[];
  /** Close the catalog. */
  close(): void;
}
