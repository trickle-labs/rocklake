/*
 * ffi_smoke.c — C ABI smoke test for the RockLake FFI layer.
 *
 * Compile:
 *   cc -I extension/include -o ffi_smoke_test \
 *       crates/rocklake-ffi/tests/ffi_smoke.c \
 *       -L target/debug -lrocklake_ffi \
 *       -Wl,-rpath,target/debug
 * Run:
 *   ./ffi_smoke_test /tmp/rl_smoke_test_catalog
 *
 * Tests the full lifecycle: open, abi_version, list_schemas (on a fresh
 * catalog at snapshot 0), error handling, close, and free.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "rocklake.h"

static int failed = 0;

#define ASSERT(cond, msg) \
    do { \
        if (!(cond)) { \
            fprintf(stderr, "FAIL [%s:%d]: %s\n", __FILE__, __LINE__, (msg)); \
            failed++; \
        } else { \
            printf("OK: %s\n", (msg)); \
        } \
    } while (0)

int main(int argc, char **argv) {
    const char *catalog_path = (argc > 1) ? argv[1] : "/tmp/rl_smoke_test";

    /* ── ABI version check ─────────────────────────────────────────────── */
    uint32_t version = rocklake_abi_version();
    ASSERT(version > 0, "abi_version returns a positive integer");

    /* ── Open catalog ──────────────────────────────────────────────────── */
    rocklake_error_t err;
    memset(&err, 0, sizeof(err));

    rocklake_catalog_t *cat = rocklake_open(catalog_path, &err);
    ASSERT(cat != NULL, "rocklake_open succeeds on a fresh directory");
    ASSERT(rocklake_error_code(&err) == ROCKLAKE_OK,
           "no error on successful open");
    rocklake_error_free(&err);

    /* ── Null URI returns error ─────────────────────────────────────────── */
    rocklake_error_t null_err;
    memset(&null_err, 0, sizeof(null_err));
    rocklake_catalog_t *null_cat = rocklake_open(NULL, &null_err);
    ASSERT(null_cat == NULL, "rocklake_open(NULL) returns NULL");
    ASSERT(rocklake_error_code(&null_err) != ROCKLAKE_OK,
           "rocklake_open(NULL) sets a non-zero error code");
    ASSERT(rocklake_error_message(&null_err) != NULL,
           "rocklake_open(NULL) provides an error message");
    rocklake_error_free(&null_err);

    if (cat == NULL) {
        fprintf(stderr, "Cannot continue without a valid catalog handle.\n");
        return 1;
    }

    /* ── list_schemas on empty catalog ─────────────────────────────────── */
    rocklake_error_t list_err;
    memset(&list_err, 0, sizeof(list_err));
    rocklake_schema_list_t schemas = rocklake_list_schemas(cat, 0, &list_err);
    ASSERT(rocklake_error_code(&list_err) == ROCKLAKE_OK,
           "list_schemas on empty catalog returns OK");
    ASSERT(schemas.count == 0,
           "empty catalog has zero schemas");
    rocklake_schema_list_free(&schemas);
    rocklake_error_free(&list_err);

    /* ── Double close is safe ───────────────────────────────────────────── */
    rocklake_close(cat);
    rocklake_close(cat);   /* second call must not crash */
    ASSERT(1, "double close does not crash");

    /* ── Close NULL is safe ─────────────────────────────────────────────── */
    rocklake_close(NULL);
    ASSERT(1, "close(NULL) does not crash");

    /* ── Summary ────────────────────────────────────────────────────────── */
    if (failed == 0) {
        printf("\nAll C ABI smoke tests passed.\n");
        return 0;
    } else {
        fprintf(stderr, "\n%d test(s) FAILED.\n", failed);
        return 1;
    }
}
