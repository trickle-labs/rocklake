/**
 * rocklake_extension.cpp — DuckDB extension ABI smoke wrapper for the
 * RockLake C FFI.
 *
 * NOTE: This file is an ABI smoke wrapper only. The ATTACH registration
 * required to make `ATTACH 'ducklake:slatedb:...' AS lake` work from DuckDB
 * is pending v0.36.0. The example below describes the planned interface and
 * is NOT yet functional.
 *
 * Planned usage in DuckDB (v0.36.0+):
 *   INSTALL rocklake;
 *   LOAD rocklake;
 *   ATTACH 'ducklake:slatedb:///path/to/catalog' AS lake;  -- planned v0.36.0
 *
 * This extension implements the DuckDB Catalog interface by delegating to the
 * Rust-based rocklake-ffi library through a stable C ABI.
 */

#include "rocklake.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>

// ─── Extension Metadata ────────────────────────────────────────────────────

static const char *EXTENSION_NAME = "rocklake";
static const char *EXTENSION_VERSION = "0.5.0";
static const uint32_t EXPECTED_ABI_VERSION = 5000;

// ─── ABI Version Check ─────────────────────────────────────────────────────

static bool verify_abi() {
    uint32_t abi = rocklake_abi_version();
    if (abi != EXPECTED_ABI_VERSION) {
        fprintf(stderr,
                "rocklake extension: ABI version mismatch. "
                "Expected %u, got %u. Please rebuild the extension.\n",
                EXPECTED_ABI_VERSION, abi);
        return false;
    }
    return true;
}

// ─── Extension Entry Point ──────────────────────────────────────────────────

/**
 * DuckDB extension entry point. Called when the extension is loaded.
 *
 * In a full DuckDB community extension, this would:
 * 1. Verify ABI version
 * 2. Register the 'slatedb' catalog type with DuckDB
 * 3. Register the ATTACH handler for 'ducklake:slatedb:' URIs
 *
 * For the beta release, we export the symbols needed for the extension
 * loading mechanism and verify the ABI.
 */
extern "C" {

#ifdef _WIN32
__declspec(dllexport)
#else
__attribute__((visibility("default")))
#endif
const char *rocklake_extension_name() {
    return EXTENSION_NAME;
}

#ifdef _WIN32
__declspec(dllexport)
#else
__attribute__((visibility("default")))
#endif
const char *rocklake_extension_version() {
    return EXTENSION_VERSION;
}

#ifdef _WIN32
__declspec(dllexport)
#else
__attribute__((visibility("default")))
#endif
bool rocklake_extension_init() {
    if (!verify_abi()) {
        return false;
    }
    // Extension loaded successfully — catalog type registration would go here
    // once DuckDB's extension catalog API is available for community extensions.
    return true;
}

} // extern "C"
