//! Path canonicalization for catalog and data paths.
//!
//! Never use raw string concatenation for object-store paths.
//! `CatalogPath` encapsulates all path logic.

use serde::{Deserialize, Serialize};

/// Mode for how data paths are stored in the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataPathMode {
    /// Absolute object-store URIs (e.g., `s3://bucket/data/warehouse-a/`).
    Absolute,
    /// Relative to the data prefix, with unambiguous `path_is_relative` flag.
    RelativeToDataPrefix,
}

/// Encapsulates all path components for a SlateDuck catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogPath {
    /// Root of the object store (e.g., `s3://bucket/`).
    pub object_store_root: String,
    /// Prefix for catalog data within the object store (e.g., `catalogs/warehouse-a/`).
    pub catalog_prefix: String,
    /// Prefix for data files (e.g., `data/warehouse-a/`).
    pub data_prefix: String,
    /// How data paths are stored.
    pub data_path_mode: DataPathMode,
}

impl CatalogPath {
    /// Create a new CatalogPath with absolute data path mode.
    pub fn new(object_store_root: &str, catalog_prefix: &str, data_prefix: &str) -> Self {
        Self {
            object_store_root: normalize_path(object_store_root),
            catalog_prefix: normalize_path(catalog_prefix),
            data_prefix: normalize_path(data_prefix),
            data_path_mode: DataPathMode::Absolute,
        }
    }

    /// Resolve a data file path to its full object-store URI.
    pub fn resolve_data_path(&self, path: &str, path_is_relative: bool) -> String {
        if !path_is_relative || self.data_path_mode == DataPathMode::Absolute {
            // Already absolute
            if path.contains("://") {
                return path.to_string();
            }
            format!("{}{}", self.object_store_root, path)
        } else {
            format!("{}{}{}", self.object_store_root, self.data_prefix, path)
        }
    }

    /// Get the full catalog path.
    pub fn catalog_uri(&self) -> String {
        format!("{}{}", self.object_store_root, self.catalog_prefix)
    }

    /// Get the full data prefix URI.
    pub fn data_uri(&self) -> String {
        format!("{}{}", self.object_store_root, self.data_prefix)
    }
}

/// Normalize a path: ensure trailing slash, no double slashes.
fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    format!("{trimmed}/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_normalization() {
        assert_eq!(normalize_path("foo/bar"), "foo/bar/");
        assert_eq!(normalize_path("foo/bar/"), "foo/bar/");
        assert_eq!(normalize_path(""), "");
    }

    #[test]
    fn resolve_absolute_path() {
        let cp = CatalogPath::new("s3://bucket/", "catalogs/wh/", "data/wh/");
        let resolved = cp.resolve_data_path("s3://bucket/data/wh/file.parquet", false);
        assert_eq!(resolved, "s3://bucket/data/wh/file.parquet");
    }

    #[test]
    fn resolve_relative_path() {
        let mut cp = CatalogPath::new("s3://bucket/", "catalogs/wh/", "data/wh/");
        cp.data_path_mode = DataPathMode::RelativeToDataPrefix;
        let resolved = cp.resolve_data_path("file.parquet", true);
        assert_eq!(resolved, "s3://bucket/data/wh/file.parquet");
    }

    #[test]
    fn catalog_uri() {
        let cp = CatalogPath::new("s3://bucket/", "catalogs/warehouse-a/", "data/warehouse-a/");
        assert_eq!(cp.catalog_uri(), "s3://bucket/catalogs/warehouse-a/");
        assert_eq!(cp.data_uri(), "s3://bucket/data/warehouse-a/");
    }
}
