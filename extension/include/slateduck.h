/**
 * slateduck.h — C ABI header for the SlateDuck catalog FFI layer.
 *
 * This header is consumed by the DuckDB extension to call into the Rust
 * catalog implementation. All types use stable C representations.
 *
 * ABI Contract: The extension MUST check slateduck_abi_version() at load
 * time and refuse to proceed on version mismatch.
 */

#ifndef SLATEDUCK_H
#define SLATEDUCK_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ─── ABI Version ──────────────────────────────────────────────────────── */

/**
 * Returns the ABI version (major * 1000 + minor).
 * Extension checks this at load time; mismatch → refuse to load.
 */
uint32_t slateduck_abi_version(void);

/* ─── Error Handling ───────────────────────────────────────────────────── */

typedef struct {
    int32_t code;
    char *message;
} slateduck_error_t;

/** Error codes */
enum {
    SLATEDUCK_OK = 0,
    SLATEDUCK_ERR_INTERNAL = 1,
    SLATEDUCK_ERR_NOT_FOUND = 2,
    SLATEDUCK_ERR_WRITER_FENCED = 3,
    SLATEDUCK_ERR_FORMAT_MISMATCH = 4,
    SLATEDUCK_ERR_VALUE_TOO_LARGE = 5,
    SLATEDUCK_ERR_TRANSACTION_CONFLICT = 6,
    SLATEDUCK_ERR_NOT_INITIALIZED = 7,
};

int32_t slateduck_error_code(const slateduck_error_t *err);
const char *slateduck_error_message(const slateduck_error_t *err);
void slateduck_error_free(slateduck_error_t *err);

/* ─── Opaque Catalog Handle ────────────────────────────────────────────── */

typedef struct SlateduckCatalog slateduck_catalog_t;

/* ─── Result Types ─────────────────────────────────────────────────────── */

typedef struct {
    uint64_t snapshot_id;
    uint64_t schema_version;
} slateduck_snapshot_t;

typedef struct {
    uint64_t data_file_id;
    uint64_t table_id;
    char *path;
    char *file_format;
    uint64_t row_count;
    uint64_t file_size_bytes;
    uint64_t snapshot_id;
} slateduck_data_file_t;

typedef struct {
    slateduck_data_file_t *files;
    uint64_t count;
} slateduck_file_list_t;

typedef struct {
    uint64_t schema_id;
    char *schema_name;
} slateduck_schema_entry_t;

typedef struct {
    slateduck_schema_entry_t *schemas;
    uint64_t count;
} slateduck_schema_list_t;

typedef struct {
    uint64_t table_id;
    uint64_t schema_id;
    char *table_name;
} slateduck_table_entry_t;

typedef struct {
    slateduck_table_entry_t *tables;
    uint64_t count;
} slateduck_table_list_t;

typedef struct {
    uint64_t column_id;
    uint64_t table_id;
    char *column_name;
    char *data_type;
    uint64_t column_index;
    bool is_nullable;
} slateduck_column_entry_t;

typedef struct {
    slateduck_column_entry_t *columns;
    uint64_t count;
} slateduck_column_list_t;

/* ─── Catalog Operations ───────────────────────────────────────────────── */

/**
 * Open a catalog at the given URI (local filesystem path).
 * Returns NULL on failure; check err for details.
 */
slateduck_catalog_t *slateduck_open(const char *uri, slateduck_error_t *err);

/** Close and free a catalog handle. */
void slateduck_close(slateduck_catalog_t *catalog);

/** Get the current (latest) snapshot. */
slateduck_snapshot_t slateduck_get_current_snapshot(
    slateduck_catalog_t *catalog, slateduck_error_t *err);

/** List schemas at a given snapshot. */
slateduck_schema_list_t slateduck_list_schemas(
    slateduck_catalog_t *catalog, uint64_t snapshot_id, slateduck_error_t *err);

/** List tables in a schema at a given snapshot. */
slateduck_table_list_t slateduck_list_tables(
    slateduck_catalog_t *catalog, uint64_t schema_id, uint64_t snapshot_id,
    slateduck_error_t *err);

/** Describe a table's columns at a given snapshot. */
slateduck_column_list_t slateduck_describe_table(
    slateduck_catalog_t *catalog, uint64_t table_id, uint64_t snapshot_id,
    slateduck_error_t *err);

/** List data files for a table at a given snapshot. */
slateduck_file_list_t slateduck_list_data_files(
    slateduck_catalog_t *catalog, uint64_t table_id, uint64_t snapshot_id,
    slateduck_error_t *err);

/* ─── Free Functions ───────────────────────────────────────────────────── */

void slateduck_schema_list_free(slateduck_schema_list_t *list);
void slateduck_table_list_free(slateduck_table_list_t *list);
void slateduck_column_list_free(slateduck_column_list_t *list);
void slateduck_file_list_free(slateduck_file_list_t *list);

#ifdef __cplusplus
}
#endif

#endif /* SLATEDUCK_H */
