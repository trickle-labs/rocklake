package io.trickle.rocklake

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Kotlin-idiomatic wrapper around RockLakeCatalog with coroutine support.
 * 
 * All blocking operations are dispatched to Dispatchers.IO to avoid blocking
 * coroutine thread pools.
 */
class RockLakeCatalogAsync(private val catalog: RockLakeCatalog) : AutoCloseable {
    
    /**
     * Gets the current snapshot ID asynchronously.
     */
    suspend fun getSnapshot(): Long = withContext(Dispatchers.IO) {
        catalog.getSnapshot()
    }

    /**
     * Lists data files asynchronously.
     */
    suspend fun listDataFiles(tableId: String): List<DataFileRow> =
        withContext(Dispatchers.IO) {
            catalog.listDataFiles(tableId)
        }

    /** Lists data files asynchronously at the selected snapshot. */
    suspend fun listDataFiles(
        tableId: String,
        snapshot: RockLakeCatalog.SnapshotRef
    ): List<DataFileRow> = withContext(Dispatchers.IO) {
        catalog.listDataFiles(tableId, snapshot)
    }

    /** Lists data files asynchronously at an exact snapshot ID. */
    suspend fun listDataFilesAt(tableId: String, snapshotId: Long): List<DataFileRow> =
        withContext(Dispatchers.IO) {
            catalog.listDataFiles(tableId, snapshotId)
        }

    /**
     * Describes a table asynchronously.
     */
    suspend fun describeTable(tableId: String): List<ColumnRow> =
        withContext(Dispatchers.IO) {
            catalog.describeTable(tableId)
        }

    /** Describes a table asynchronously at the selected snapshot. */
    suspend fun describeTable(
        tableId: String,
        snapshot: RockLakeCatalog.SnapshotRef
    ): List<ColumnRow> = withContext(Dispatchers.IO) {
        catalog.describeTable(tableId, snapshot)
    }

    /** Describes a table asynchronously at an exact snapshot ID. */
    suspend fun describeTableAt(tableId: String, snapshotId: Long): List<ColumnRow> =
        withContext(Dispatchers.IO) {
            catalog.describeTable(tableId, snapshotId)
        }

    /**
     * Creates a snapshot asynchronously.
     */
    suspend fun createSnapshot(changes: String = ""): Long = withContext(Dispatchers.IO) {
        catalog.createSnapshot(changes)
    }

    /**
     * Closes the catalog asynchronously.
     */
    override fun close() {
        catalog.close()
    }

    /**
     * Checks if the catalog is still open.
     */
    fun isOpen(): Boolean = catalog.isOpen()

    /**
     * Gets the catalog path.
     */
    fun getPath(): String = catalog.path
}

/**
 * Extension function to create an async wrapper from a RockLakeCatalog.
 */
fun RockLakeCatalog.async(): RockLakeCatalogAsync = RockLakeCatalogAsync(this)

/**
 * Extension function for use-site resource management with async operations.
 * 
 * Example:
 * ```
 * RockLakeCatalog("/path/to/catalog").async().use { async ->
 *     val snapshot = async.getSnapshot()
 *     val files = async.listDataFiles("my_table")
 * }
 * ```
 */
suspend fun <T> RockLakeCatalogAsync.use(block: suspend (RockLakeCatalogAsync) -> T): T {
    return try {
        block(this)
    } finally {
        close()
    }
}
