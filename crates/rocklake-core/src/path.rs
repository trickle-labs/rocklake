//! Catalog path canonicalization.
//!
//! Never use raw string concatenation for object-store paths anywhere.

use thiserror::Error;

/// Error returned when a catalog path cannot be resolved safely.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathError {
    /// The path would escape its configured root or otherwise disagree with its metadata.
    #[error("invalid path: {0}")]
    Invalid(String),
}

/// Determine if a path is relative (no scheme or local absolute path).
///
/// Returns `true` if the path is relative (should use `path_is_relative = true`),
/// `false` if it is a URI or a local absolute path.
pub fn is_path_relative(path: &str) -> bool {
    !path.contains("://") && !std::path::Path::new(path).is_absolute()
}

/// Validate a prefix that is relative to an object-store bucket or container.
pub fn validate_object_prefix(prefix: &str) -> Result<(), PathError> {
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() {
        return Ok(());
    }
    if prefix
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(PathError::Invalid(format!(
            "object-store prefix '{prefix}' contains an ambiguous component"
        )));
    }
    Ok(())
}

/// Mode for data path storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataPathMode {
    /// Absolute object-store URI (e.g., `s3://bucket/data/warehouse-a/`).
    Absolute,
    /// Relative to the data prefix, with explicit `path_is_relative` flag.
    RelativeToDataPrefix,
}

/// Encapsulates all path components for a RockLake catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPath {
    /// Root of the object store (e.g., `s3://bucket/`).
    pub object_store_root: String,
    /// Prefix under which catalog (SlateDB) data is stored.
    pub catalog_prefix: String,
    /// Prefix under which Parquet data files are stored.
    pub data_prefix: String,
    /// How data paths are stored in the catalog.
    pub data_path_mode: DataPathMode,
}

impl CatalogPath {
    /// Create a new CatalogPath with all components.
    pub fn new(
        object_store_root: impl Into<String>,
        catalog_prefix: impl Into<String>,
        data_prefix: impl Into<String>,
        data_path_mode: DataPathMode,
    ) -> Self {
        let root = normalize_trailing_slash(object_store_root.into());
        let catalog = normalize_trailing_slash(catalog_prefix.into());
        let data = normalize_trailing_slash(data_prefix.into());
        Self {
            object_store_root: root,
            catalog_prefix: catalog,
            data_prefix: data,
            data_path_mode,
        }
    }

    /// Resolve a data file path to its full object-store URI.
    pub fn resolve_data_path(&self, stored_path: &str) -> String {
        self.resolve_data_path_checked(stored_path)
            .unwrap_or_else(|_| match self.data_path_mode {
                DataPathMode::Absolute => stored_path.to_string(),
                DataPathMode::RelativeToDataPrefix => format!(
                    "{}{}",
                    self.data_prefix,
                    stored_path.trim_start_matches('/')
                ),
            })
    }

    /// Resolve a data path and reject traversal or a root mismatch.
    pub fn resolve_data_path_checked(&self, stored_path: &str) -> Result<String, PathError> {
        let relative = is_path_relative(stored_path);
        match self.data_path_mode {
            DataPathMode::Absolute => {
                if relative {
                    return Err(PathError::Invalid(format!(
                        "relative path '{stored_path}' in absolute mode"
                    )));
                }
                validate_absolute_path(&self.data_prefix, stored_path)
            }
            DataPathMode::RelativeToDataPrefix => {
                if !relative {
                    return Err(PathError::Invalid(format!(
                        "absolute path '{stored_path}' in relative mode"
                    )));
                }
                join_checked(&self.data_prefix, stored_path)
            }
        }
    }

    /// Convert an absolute data path to its stored form.
    pub fn to_stored_path(&self, absolute_path: &str) -> String {
        self.to_stored_path_checked(absolute_path)
            .unwrap_or_else(|_| absolute_path.to_string())
    }

    /// Convert an absolute data path to its stored form, validating its root.
    pub fn to_stored_path_checked(&self, absolute_path: &str) -> Result<String, PathError> {
        match self.data_path_mode {
            DataPathMode::Absolute => validate_absolute_path(&self.data_prefix, absolute_path),
            DataPathMode::RelativeToDataPrefix => {
                let path = ensure_prefix(&self.data_prefix, absolute_path)?;
                Ok(path
                    .strip_prefix(self.data_prefix.trim_matches('/'))
                    .unwrap_or(&path)
                    .trim_start_matches('/')
                    .to_string())
            }
        }
    }

    /// Full path to the catalog (SlateDB) directory.
    pub fn catalog_full_path(&self) -> String {
        format!(
            "{}{}",
            self.object_store_root,
            self.catalog_prefix.trim_start_matches('/')
        )
    }

    /// Full path to the data directory.
    pub fn data_full_path(&self) -> String {
        format!(
            "{}{}",
            self.object_store_root,
            self.data_prefix.trim_start_matches('/')
        )
    }
}

/// Resolve a catalog row path into the object-store namespace used by cleanup.
pub fn resolve_object_path(
    data_prefix: &str,
    stored_path: &str,
    path_is_relative: Option<bool>,
) -> Result<String, PathError> {
    let inferred = is_path_relative(stored_path);
    let relative = path_is_relative.unwrap_or(inferred);
    if path_is_relative.is_some() && relative != inferred {
        return Err(PathError::Invalid(format!(
            "path '{stored_path}' disagrees with path_is_relative"
        )));
    }
    if relative {
        join_checked(data_prefix, stored_path)
    } else {
        let path = stored_path
            .split_once("://")
            .map(|(_, rest)| rest.split_once('/').map_or("", |(_, path)| path))
            .unwrap_or_else(|| stored_path.trim_start_matches('/'));
        ensure_prefix(data_prefix, path)
    }
}

/// Resolve a catalog data-file path against a configured data root.
pub fn resolve_data_path(
    data_root: &str,
    stored_path: &str,
    path_is_relative: Option<bool>,
) -> Result<String, PathError> {
    let inferred = is_path_relative(stored_path);
    let relative = path_is_relative.unwrap_or(inferred);
    if path_is_relative.is_some() && relative != inferred {
        return Err(PathError::Invalid(format!(
            "path '{stored_path}' disagrees with path_is_relative"
        )));
    }
    if stored_path.trim_matches('/').is_empty() {
        return Err(PathError::Invalid("empty data-file path".to_string()));
    }
    if relative {
        return join_data_root(data_root, stored_path);
    }

    if data_root.contains("://") && !stored_path.contains("://") {
        return Err(PathError::Invalid(format!(
            "absolute path '{stored_path}' does not use data root '{data_root}'"
        )));
    }
    if data_root.starts_with('/') && stored_path.contains("://") {
        return Err(PathError::Invalid(format!(
            "absolute path '{stored_path}' does not use local data root '{data_root}'"
        )));
    }
    validate_absolute_path(data_root, stored_path)
}

fn join_data_root(root: &str, path: &str) -> Result<String, PathError> {
    if path.split('/').any(|part| part == "..") {
        return Err(PathError::Invalid(format!(
            "path '{path}' contains traversal"
        )));
    }
    let path = path.trim_matches('/');
    if root.starts_with('/') {
        return Ok(std::path::Path::new(root)
            .join(path)
            .to_string_lossy()
            .into_owned());
    }
    let root = root.trim_end_matches('/');
    Ok(if root.is_empty() {
        path.to_string()
    } else {
        format!("{root}/{path}")
    })
}

fn join_checked(prefix: &str, path: &str) -> Result<String, PathError> {
    let prefix = prefix.trim_matches('/');
    let path = path.trim_matches('/');
    if path.is_empty() {
        return Err(PathError::Invalid("empty path".to_string()));
    }
    if path.split('/').any(|part| part == "..") {
        return Err(PathError::Invalid(format!(
            "path '{path}' contains traversal"
        )));
    }
    ensure_prefix(prefix, &format!("{prefix}/{path}"))
}

fn validate_absolute_path(prefix: &str, path: &str) -> Result<String, PathError> {
    let original = path;
    let (path, path_root) = uri_path(path);
    let (prefix, prefix_root) = uri_path(prefix);
    if let (Some(path_root), Some(prefix_root)) = (path_root, prefix_root) {
        if path_root != prefix_root {
            return Err(PathError::Invalid(format!(
                "path root '{path_root:?}' does not match '{prefix_root:?}'"
            )));
        }
    }
    ensure_prefix(prefix, path).map(|_| original.to_string())
}

fn uri_path(path: &str) -> (&str, Option<(&str, &str)>) {
    path.split_once("://")
        .map_or((path.trim_start_matches('/'), None), |(scheme, rest)| {
            rest.split_once('/')
                .map_or(("", Some((scheme, rest))), |(root, path)| {
                    (path, Some((scheme, root)))
                })
        })
}

fn ensure_prefix(prefix: &str, path: &str) -> Result<String, PathError> {
    if path.split('/').any(|part| part == "..") {
        return Err(PathError::Invalid(format!(
            "path '{path}' contains traversal"
        )));
    }
    let prefix = prefix.trim_matches('/');
    let path = path.trim_matches('/');
    if !prefix.is_empty()
        && path != prefix
        && !path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
    {
        return Err(PathError::Invalid(format!(
            "path '{path}' is outside data prefix '{prefix}'"
        )));
    }
    Ok(path.to_string())
}

/// Ensure a path ends with a slash.
fn normalize_trailing_slash(mut s: String) -> String {
    if !s.is_empty() && !s.ends_with('/') {
        s.push('/');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_mode_resolve() {
        let cp = CatalogPath::new(
            "s3://mybucket",
            "catalogs/main",
            "data/warehouse",
            DataPathMode::Absolute,
        );
        let path = "s3://mybucket/data/warehouse/table1/file.parquet";
        assert_eq!(cp.resolve_data_path(path), path);
    }

    #[test]
    fn relative_mode_resolve() {
        let cp = CatalogPath::new(
            "s3://mybucket",
            "catalogs/main",
            "s3://mybucket/data/warehouse",
            DataPathMode::RelativeToDataPrefix,
        );
        assert_eq!(
            cp.resolve_data_path("table1/file.parquet"),
            "s3://mybucket/data/warehouse/table1/file.parquet"
        );
    }

    #[test]
    fn to_stored_path_relative() {
        let cp = CatalogPath::new(
            "s3://mybucket",
            "catalogs/main",
            "s3://mybucket/data/warehouse",
            DataPathMode::RelativeToDataPrefix,
        );
        assert_eq!(
            cp.to_stored_path("s3://mybucket/data/warehouse/table1/file.parquet"),
            "table1/file.parquet"
        );
    }

    #[test]
    fn trailing_slash_normalized() {
        let cp = CatalogPath::new("s3://bucket", "catalog", "data", DataPathMode::Absolute);
        assert!(cp.object_store_root.ends_with('/'));
        assert!(cp.catalog_prefix.ends_with('/'));
        assert!(cp.data_prefix.ends_with('/'));
    }

    #[test]
    fn catalog_full_path() {
        let cp = CatalogPath::new(
            "s3://mybucket",
            "catalogs/main",
            "data/warehouse",
            DataPathMode::Absolute,
        );
        assert_eq!(cp.catalog_full_path(), "s3://mybucket/catalogs/main/");
    }

    #[test]
    fn path_relativity_detection() {
        // Relative paths (no scheme)
        assert!(is_path_relative("table/file.parquet"));
        assert!(is_path_relative("data/orders/part-00042.parquet"));
        assert!(is_path_relative("../relative/path.parquet"));

        // Absolute paths (with scheme)
        assert!(!is_path_relative("s3://bucket/table/file.parquet"));
        assert!(!is_path_relative("az://container/table/file.parquet"));
        assert!(!is_path_relative("gs://bucket/data/file.parquet"));
        assert!(!is_path_relative("file:///local/path/file.parquet"));
        assert!(!is_path_relative("/local/path/file.parquet"));
    }

    #[test]
    fn checked_paths_preserve_nested_prefixes_and_reject_escape() {
        let cp = CatalogPath::new(
            "s3://bucket",
            "catalog/main",
            "data/warehouse/nested",
            DataPathMode::RelativeToDataPrefix,
        );
        assert_eq!(
            cp.resolve_data_path_checked("table/file.parquet").unwrap(),
            "data/warehouse/nested/table/file.parquet"
        );
        assert!(cp.resolve_data_path_checked("../outside.parquet").is_err());
        assert!(cp
            .resolve_data_path_checked("s3://other/data/file.parquet")
            .is_err());
        assert!(cp
            .to_stored_path_checked("data/warehouse/nested/table/file.parquet")
            .is_ok());
        assert!(cp
            .to_stored_path_checked("data/warehouse/nested2/file.parquet")
            .is_err());
    }

    #[test]
    fn checked_absolute_paths_require_the_same_root() {
        let cp = CatalogPath::new(
            "s3://bucket",
            "catalog/main",
            "s3://bucket/data/warehouse",
            DataPathMode::Absolute,
        );
        assert!(cp
            .resolve_data_path_checked("s3://other/data/warehouse/file.parquet")
            .is_err());
        assert_eq!(
            cp.resolve_data_path_checked("s3://bucket/data/warehouse/file.parquet")
                .unwrap(),
            "s3://bucket/data/warehouse/file.parquet"
        );
    }

    #[test]
    fn object_paths_use_the_store_namespace() {
        assert_eq!(
            resolve_object_path("data/warehouse/nested", "table/file.parquet", Some(true)).unwrap(),
            "data/warehouse/nested/table/file.parquet"
        );
        assert_eq!(
            resolve_object_path(
                "data/warehouse/nested",
                "s3://bucket/data/warehouse/nested/table/file.parquet",
                Some(false),
            )
            .unwrap(),
            "data/warehouse/nested/table/file.parquet"
        );
        assert!(resolve_object_path("data/warehouse", "../outside", Some(true)).is_err());
        assert!(resolve_object_path("data/warehouse", "data/other/file", Some(false)).is_err());
    }

    #[test]
    fn data_paths_keep_uri_and_local_roots() {
        assert_eq!(
            resolve_data_path(
                "s3://bucket/data/warehouse",
                "table/file.parquet",
                Some(true)
            )
            .unwrap(),
            "s3://bucket/data/warehouse/table/file.parquet"
        );
        assert_eq!(
            resolve_data_path(
                "/tmp/data/warehouse",
                "/tmp/data/warehouse/table/file.parquet",
                Some(false),
            )
            .unwrap(),
            "/tmp/data/warehouse/table/file.parquet"
        );
        assert!(resolve_data_path("s3://bucket/data", "../outside", Some(true)).is_err());
        assert!(resolve_data_path(
            "s3://bucket/data",
            "s3://other/data/file.parquet",
            Some(false),
        )
        .is_err());
    }

    #[test]
    fn object_store_prefixes_reject_ambiguous_components() {
        assert!(validate_object_prefix("tenant/warehouse").is_ok());
        assert!(validate_object_prefix("tenant/../outside").is_err());
        assert!(validate_object_prefix("tenant//warehouse").is_err());
    }
}
