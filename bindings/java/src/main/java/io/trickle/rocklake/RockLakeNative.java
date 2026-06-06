package io.trickle.rocklake;

import com.sun.jna.Library;
import com.sun.jna.Native;
import com.sun.jna.Pointer;
import com.sun.jna.Structure;

import java.io.File;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;

/**
 * Java binding to the native RockLake C ABI.
 */
public class RockLakeNative {
    private static volatile RockLakeLibrary library;

    static synchronized void loadLibrary() {
        if (library != null) {
            return;
        }

        String libName = getLibraryName();
        try {
            library = loadWorkspaceLibrary();
            if (library == null) {
                library = loadResourceLibrary(libName);
            }
            if (library == null) {
                library = Native.load("rocklake_ffi", RockLakeLibrary.class);
            }
        } catch (UnsatisfiedLinkError e) {
            throw new ExceptionInInitializerError("Failed to load RockLake native library: " + e.getMessage());
        }
    }

    private static RockLakeLibrary loadWorkspaceLibrary() {
        String userDir = System.getProperty("user.dir");
        Path workspaceRoot = new File(userDir).toPath().resolve("..").resolve("..").normalize();
        Path releasePath = workspaceRoot.resolve("target").resolve("release").resolve(nativeLibraryFileName());
        Path debugPath = workspaceRoot.resolve("target").resolve("debug").resolve(nativeLibraryFileName());

        if (Files.exists(releasePath)) {
            return Native.load(releasePath.toAbsolutePath().toString(), RockLakeLibrary.class);
        }

        if (Files.exists(debugPath)) {
            return Native.load(debugPath.toAbsolutePath().toString(), RockLakeLibrary.class);
        }

        return null;
    }

    private static RockLakeLibrary loadResourceLibrary(String libName) {
        String resourcePath = "/native/" + libName;
        try (InputStream in = RockLakeNative.class.getResourceAsStream(resourcePath)) {
            if (in == null) {
                return null;
            }

            File tempFile = File.createTempFile("rocklake",
                System.getProperty("os.name").toLowerCase().contains("windows") ? ".dll" :
                System.getProperty("os.name").toLowerCase().contains("mac") ? ".dylib" : ".so");
            tempFile.deleteOnExit();

            try (OutputStream out = Files.newOutputStream(tempFile.toPath())) {
                byte[] buffer = new byte[8192];
                int read;
                while ((read = in.read(buffer)) != -1) {
                    out.write(buffer, 0, read);
                }
            }

            return Native.load(tempFile.getAbsolutePath(), RockLakeLibrary.class);
        } catch (Exception e) {
            return null;
        }
    }

    private static String getLibraryName() {
        String osName = System.getProperty("os.name").toLowerCase();
        String osArch = System.getProperty("os.arch").toLowerCase();
        String libExtension;

        if (osName.contains("windows")) {
            libExtension = ".dll";
        } else if (osName.contains("mac")) {
            libExtension = ".dylib";
        } else {
            libExtension = ".so";
        }

        String arch;
        if (osArch.contains("amd64") || osArch.contains("x86_64")) {
            arch = "x86_64";
        } else if (osArch.contains("aarch64") || osArch.contains("arm64")) {
            arch = "arm64";
        } else {
            arch = osArch;
        }

        String platform;
        if (osName.contains("windows")) {
            platform = "windows-" + arch;
        } else if (osName.contains("mac")) {
            platform = "macos-" + arch;
        } else if (osName.contains("linux")) {
            platform = "linux-" + arch;
        } else {
            platform = osName + "-" + arch;
        }

        return "rocklake-" + platform + libExtension;
    }

    private static String nativeLibraryFileName() {
        String osName = System.getProperty("os.name").toLowerCase();
        if (osName.contains("windows")) {
            return "rocklake_ffi.dll";
        }
        if (osName.contains("mac")) {
            return "librocklake_ffi.dylib";
        }
        return "librocklake_ffi.so";
    }

    private static Pointer toPointer(long handle) {
        return handle == 0 ? Pointer.NULL : Pointer.createConstant(handle);
    }

    private static RockLakeError.ByReference newError() {
        RockLakeError.ByReference err = new RockLakeError.ByReference();
        err.code = 0;
        err.message = null;
        err.write();
        return err;
    }

    private static void freeError(RockLakeError.ByReference err) {
        if (library != null && err != null) {
            library.rocklake_error_free(err);
        }
    }

    private static String errorMessage(RockLakeError.ByReference err) {
        err.read();
        if (err.message == null) {
            return "";
        }
        return err.message.getString(0);
    }

    private static void throwIfFailure(String operation, RockLakeError.ByReference err) throws RockLakeException {
        err.read();
        if (err.code != 0) {
            String message = errorMessage(err);
            freeError(err);
            throw new RockLakeException(operation + ": " + message + " (code " + err.code + ")");
        }
        freeError(err);
    }

    static long openCatalog(String path, Map<String, String> options) throws RockLakeException {
        if (path == null || path.isEmpty()) {
            throw new IllegalArgumentException("path cannot be null or empty");
        }

        RockLakeError.ByReference err = newError();
        Pointer ptr = library.rocklake_open(path, err);
        err.read();
        String message = errorMessage(err);
        int code = err.code;
        freeError(err);

        if (ptr == null) {
            throw new RockLakeException("openCatalog: " + message + " (code " + code + ")");
        }
        return Pointer.nativeValue(ptr);
    }

    static long openCatalogReadOnly(String path) throws RockLakeException {
        if (path == null || path.isEmpty()) {
            throw new IllegalArgumentException("path cannot be null or empty");
        }

        RockLakeError.ByReference err = newError();
        Pointer ptr = library.rocklake_open_readonly(path, err);
        err.read();
        String message = errorMessage(err);
        int code = err.code;
        freeError(err);

        if (ptr == null) {
            throw new RockLakeException("openCatalogReadOnly: " + message + " (code " + code + ")");
        }
        return Pointer.nativeValue(ptr);
    }

    static long getSnapshot(long handle) throws RockLakeException {
        RockLakeError.ByReference err = newError();
        RockLakeSnapshot snapshot = library.rocklake_get_current_snapshot(toPointer(handle), err);
        throwIfFailure("getSnapshot", err);
        return snapshot.snapshotId;
    }

    static List<DataFileRow> listDataFiles(long handle, String tableId, long snapshotId) throws RockLakeException {
        RockLakeError.ByReference err = newError();
        RockLakeFileList list = library.rocklake_list_data_files(toPointer(handle), parseUnsigned(tableId), snapshotId, err);
        try {
            throwIfFailure("listDataFiles", err);
            if (list.count <= 0 || list.files == null) {
                return Collections.emptyList();
            }

            RockLakeDataFile entry = new RockLakeDataFile(list.files);
            RockLakeDataFile[] entries = (RockLakeDataFile[]) entry.toArray((int) list.count);
            List<DataFileRow> rows = new ArrayList<>(entries.length);
            for (RockLakeDataFile file : entries) {
                file.read();
                rows.add(new DataFileRow(
                    file.dataFileId,
                    file.path == null ? null : file.path.getString(0),
                    file.fileSizeBytes,
                    file.rowCount,
                    0,
                    0,
                    file.snapshotId
                ));
            }
            return rows;
        } finally {
            freeFileList(list);
        }
    }

    static List<ColumnRow> describeTable(long handle, String tableId, long snapshotId) throws RockLakeException {
        RockLakeError.ByReference err = newError();
        RockLakeColumnList list = library.rocklake_describe_table(toPointer(handle), parseUnsigned(tableId), snapshotId, err);
        try {
            throwIfFailure("describeTable", err);
            if (list.count <= 0 || list.columns == null) {
                return Collections.emptyList();
            }

            RockLakeColumn entry = new RockLakeColumn(list.columns);
            RockLakeColumn[] entries = (RockLakeColumn[]) entry.toArray((int) list.count);
            List<ColumnRow> rows = new ArrayList<>(entries.length);
            for (RockLakeColumn column : entries) {
                column.read();
                rows.add(new ColumnRow(
                    (int) column.columnId,
                    column.columnName == null ? null : column.columnName.getString(0),
                    column.dataType == null ? null : column.dataType.getString(0),
                    column.isNullable,
                    null
                ));
            }
            return rows;
        } finally {
            freeColumnList(list);
        }
    }

    static long createSnapshot(long handle, String changes) throws RockLakeException {
        RockLakeError.ByReference err = newError();
        long snapshotId = library.rocklake_create_snapshot(toPointer(handle), changes == null ? "" : changes, err);
        throwIfFailure("createSnapshot", err);
        return snapshotId;
    }

    static void closeCatalog(long handle) throws RockLakeException {
        library.rocklake_close(toPointer(handle));
    }

    private static long parseUnsigned(String value) throws RockLakeException {
        try {
            return Long.parseUnsignedLong(value);
        } catch (NumberFormatException e) {
            throw new RockLakeException("Invalid numeric identifier: " + value, e);
        }
    }

    private static void freeFileList(RockLakeFileList list) {
        if (library == null) {
            return;
        }
        RockLakeFileList.ByReference copy = new RockLakeFileList.ByReference();
        copy.files = list.files;
        copy.count = list.count;
        copy.write();
        library.rocklake_file_list_free(copy);
    }

    private static void freeColumnList(RockLakeColumnList list) {
        if (library == null) {
            return;
        }
        RockLakeColumnList.ByReference copy = new RockLakeColumnList.ByReference();
        copy.columns = list.columns;
        copy.count = list.count;
        copy.write();
        library.rocklake_column_list_free(copy);
    }

    public interface RockLakeLibrary extends Library {
        int rocklake_abi_version();

        Pointer rocklake_open(String uri, RockLakeError.ByReference err);

        Pointer rocklake_open_readonly(String uri, RockLakeError.ByReference err);

        void rocklake_close(Pointer catalog);

        RockLakeSnapshot rocklake_get_current_snapshot(Pointer catalog, RockLakeError.ByReference err);

        RockLakeSchemaList rocklake_list_schemas(Pointer catalog, long snapshotId, RockLakeError.ByReference err);

        RockLakeTableList rocklake_list_tables(Pointer catalog, long schemaId, long snapshotId, RockLakeError.ByReference err);

        RockLakeColumnList rocklake_describe_table(Pointer catalog, long tableId, long snapshotId, RockLakeError.ByReference err);

        RockLakeFileList rocklake_list_data_files(Pointer catalog, long tableId, long snapshotId, RockLakeError.ByReference err);

        long rocklake_create_snapshot(Pointer catalog, String changes, RockLakeError.ByReference err);

        void rocklake_error_free(RockLakeError.ByReference err);

        void rocklake_schema_list_free(RockLakeSchemaList.ByReference list);

        void rocklake_table_list_free(RockLakeTableList.ByReference list);

        void rocklake_column_list_free(RockLakeColumnList.ByReference list);

        void rocklake_file_list_free(RockLakeFileList.ByReference list);
    }

    public static class RockLakeError extends Structure {
        public int code;
        public Pointer message;

        public static class ByReference extends RockLakeError implements Structure.ByReference {
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("code", "message");
        }
    }

    public static class RockLakeSnapshot extends Structure implements Structure.ByValue {
        public long snapshotId;
        public long schemaVersion;

        @Override
        protected List<String> getFieldOrder() {
            return List.of("snapshotId", "schemaVersion");
        }
    }

    public static class RockLakeDataFile extends Structure {
        public long dataFileId;
        public long tableId;
        public Pointer path;
        public Pointer fileFormat;
        public long rowCount;
        public long fileSizeBytes;
        public long snapshotId;

        RockLakeDataFile() {
        }

        RockLakeDataFile(Pointer pointer) {
            super(pointer);
            read();
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("dataFileId", "tableId", "path", "fileFormat", "rowCount", "fileSizeBytes", "snapshotId");
        }
    }

    public static class RockLakeFileList extends Structure implements Structure.ByValue {
        public Pointer files;
        public long count;

        public static class ByReference extends RockLakeFileList implements Structure.ByReference {
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("files", "count");
        }
    }

    public static class RockLakeSchemaEntry extends Structure {
        public long schemaId;
        public Pointer schemaName;

        RockLakeSchemaEntry() {
        }

        RockLakeSchemaEntry(Pointer pointer) {
            super(pointer);
            read();
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("schemaId", "schemaName");
        }
    }

    public static class RockLakeSchemaList extends Structure implements Structure.ByValue {
        public Pointer schemas;
        public long count;

        public static class ByReference extends RockLakeSchemaList implements Structure.ByReference {
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("schemas", "count");
        }
    }

    public static class RockLakeTableEntry extends Structure {
        public long tableId;
        public long schemaId;
        public Pointer tableName;

        RockLakeTableEntry() {
        }

        RockLakeTableEntry(Pointer pointer) {
            super(pointer);
            read();
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("tableId", "schemaId", "tableName");
        }
    }

    public static class RockLakeTableList extends Structure implements Structure.ByValue {
        public Pointer tables;
        public long count;

        public static class ByReference extends RockLakeTableList implements Structure.ByReference {
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("tables", "count");
        }
    }

    public static class RockLakeColumn extends Structure {
        public long columnId;
        public long tableId;
        public Pointer columnName;
        public Pointer dataType;
        public long columnIndex;
        public boolean isNullable;

        RockLakeColumn() {
        }

        RockLakeColumn(Pointer pointer) {
            super(pointer);
            read();
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("columnId", "tableId", "columnName", "dataType", "columnIndex", "isNullable");
        }
    }

    public static class RockLakeColumnList extends Structure implements Structure.ByValue {
        public Pointer columns;
        public long count;

        public static class ByReference extends RockLakeColumnList implements Structure.ByReference {
        }

        @Override
        protected List<String> getFieldOrder() {
            return List.of("columns", "count");
        }
    }
}
