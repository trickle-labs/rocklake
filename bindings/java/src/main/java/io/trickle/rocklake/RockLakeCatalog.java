package io.trickle.rocklake;

import java.util.List;
import java.util.Map;
import java.util.Collections;

/**
 * High-level Java API for RockLake catalog operations.
 * 
 * This class provides a user-friendly interface to the native RockLake C ABI
 * through JNI. All operations are backed by the native rocklake.h functions.
 */
public class RockLakeCatalog implements AutoCloseable {
    static {
        RockLakeNative.loadLibrary();
    }

    private long handle;
    private String path;
    private volatile boolean closed = false;

    /** Selects the latest committed snapshot or an exact snapshot ID. */
    public static final class SnapshotRef {
        private final Long snapshotId;

        private SnapshotRef(Long snapshotId) {
            this.snapshotId = snapshotId;
        }

        /** Select the latest committed snapshot. */
        public static SnapshotRef latest() {
            return new SnapshotRef(null);
        }

        /** Select an exact snapshot ID. Snapshot ID 0 is an exact ID. */
        public static SnapshotRef at(long snapshotId) {
            return new SnapshotRef(snapshotId);
        }

        private long resolve(RockLakeCatalog catalog) throws RockLakeException {
            return snapshotId != null ? snapshotId : catalog.getSnapshot();
        }
    }

    /**
     * Opens or creates a RockLake catalog at the given path.
     * 
     * @param path the catalog path (e.g., "s3://bucket/catalog" or "/local/path")
     * @param options optional configuration map (e.g., {"object_store": "s3"})
     * @throws RockLakeException if opening fails
     */
    public RockLakeCatalog(String path, Map<String, String> options) throws RockLakeException {
        if (path == null || path.isEmpty()) {
            throw new IllegalArgumentException("path cannot be null or empty");
        }
        this.path = path;
        this.handle = RockLakeNative.openCatalog(path, options != null ? options : Collections.emptyMap());
    }

    /**
     * Opens or creates a RockLake catalog at the given path with default options.
     * 
     * @param path the catalog path
     * @throws RockLakeException if opening fails
     */
    public RockLakeCatalog(String path) throws RockLakeException {
        this(path, null);
    }

    /**
     * Opens a RockLake catalog in read-only mode (no writer epoch acquired).
     *
     * Use this for stateless reader replicas and analytics sidecars.
     * Many simultaneous {@code openReadOnly} calls against the same catalog
     * path produce zero CAS write conflicts.
     *
     * @param path the catalog path
     * @return a RockLakeCatalog in read-only mode
     * @throws RockLakeException if opening fails
     */
    public static RockLakeCatalog openReadOnly(String path) throws RockLakeException {
        if (path == null || path.isEmpty()) {
            throw new IllegalArgumentException("path cannot be null or empty");
        }
        RockLakeCatalog cat = new RockLakeCatalog();
        cat.path = path;
        cat.handle = RockLakeNative.openCatalogReadOnly(path);
        return cat;
    }

    /** Private no-arg constructor used by openReadOnly. */
    private RockLakeCatalog() {
        this.path = "";
        this.handle = 0;
    }

    /**
     * Gets the current snapshot ID.
     * 
     * @return the snapshot ID
     * @throws RockLakeException if the operation fails
     */
    public long getSnapshot() throws RockLakeException {
        checkClosed();
        return RockLakeNative.getSnapshot(handle);
    }

    /**
     * Lists all data files for a table at an exact snapshot ID.
     * 
     * @param tableId the table ID
     * @param snapshotId the snapshot ID
     * @return list of DataFileRow objects
     * @throws RockLakeException if the operation fails
     */
    public List<DataFileRow> listDataFiles(String tableId, long snapshotId) throws RockLakeException {
        checkClosed();
        return RockLakeNative.listDataFiles(handle, tableId, snapshotId);
    }

    /** Lists all data files for a table at the selected snapshot. */
    public List<DataFileRow> listDataFiles(String tableId, SnapshotRef snapshot)
            throws RockLakeException {
        if (snapshot == null) {
            throw new IllegalArgumentException("snapshot cannot be null");
        }
        return listDataFiles(tableId, snapshot.resolve(this));
    }

    /**
     * Lists all data files for a table at the current snapshot.
     * 
     * @param tableId the table ID
     * @return list of DataFileRow objects
     * @throws RockLakeException if the operation fails
     */
    public List<DataFileRow> listDataFiles(String tableId) throws RockLakeException {
        return listDataFiles(tableId, SnapshotRef.latest());
    }

    /**
     * Describes a table schema (columns) at an exact snapshot ID.
     * 
     * @param tableId the table ID
     * @param snapshotId the snapshot ID
     * @return list of ColumnRow objects representing the table schema
     * @throws RockLakeException if the operation fails
     */
    public List<ColumnRow> describeTable(String tableId, long snapshotId) throws RockLakeException {
        checkClosed();
        return RockLakeNative.describeTable(handle, tableId, snapshotId);
    }

    /** Describes a table schema at the selected snapshot. */
    public List<ColumnRow> describeTable(String tableId, SnapshotRef snapshot)
            throws RockLakeException {
        if (snapshot == null) {
            throw new IllegalArgumentException("snapshot cannot be null");
        }
        return describeTable(tableId, snapshot.resolve(this));
    }

    /**
     * Describes a table schema at the current snapshot.
     * 
     * @param tableId the table ID
     * @return list of ColumnRow objects
     * @throws RockLakeException if the operation fails
     */
    public List<ColumnRow> describeTable(String tableId) throws RockLakeException {
        return describeTable(tableId, SnapshotRef.latest());
    }

    /**
     * Creates a new snapshot with the given changes.
     * 
     * @param changes a snapshot change description (format depends on RockLake)
     * @return the new snapshot ID
     * @throws RockLakeException if the operation fails
     */
    public long createSnapshot(String changes) throws RockLakeException {
        checkClosed();
        return RockLakeNative.createSnapshot(handle, changes != null ? changes : "");
    }

    /**
     * Closes the catalog, releasing all native resources.
     * 
     * @throws RockLakeException if closing fails
     */
    @Override
    public void close() throws RockLakeException {
        if (!closed) {
            synchronized (this) {
                if (!closed) {
                    RockLakeNative.closeCatalog(handle);
                    closed = true;
                }
            }
        }
    }

    /**
     * Checks if the catalog is still open.
     * 
     * @return true if the catalog is open, false otherwise
     */
    public boolean isOpen() {
        return !closed;
    }

    /**
     * Gets the catalog path.
     * 
     * @return the catalog path
     */
    public String getPath() {
        return path;
    }

    private void checkClosed() throws RockLakeException {
        if (closed) {
            throw new RockLakeException("Catalog is closed");
        }
    }
}
