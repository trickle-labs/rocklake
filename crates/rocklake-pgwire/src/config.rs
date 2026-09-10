//! Typed `rocklake.toml` configuration for the supported binary.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    pub catalog: Option<String>,
    pub router: Option<rocklake_router::RouterSettings>,
    pub registry: Option<rocklake_router::RegistrySettings>,
    #[serde(default)]
    pub catalogs: Vec<rocklake_router::CatalogConfig>,
    /// Multi-principal SCRAM verifier records.
    #[serde(default)]
    pub principals: Vec<rocklake_router::PrincipalRecord>,
    /// Per-catalog permissions for configured principals.
    #[serde(default)]
    pub grants: Vec<rocklake_router::CatalogGrant>,
    pub bind: Option<String>,
    pub max_sessions: Option<usize>,
    pub metrics_port: Option<u16>,
    pub metrics_path: Option<String>,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub tls_required: Option<bool>,
    pub auth_user: Option<String>,
    pub auth_password: Option<String>,
    pub auth_password_file: Option<String>,
    pub mode: Option<String>,
    pub cost_mode: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_path_style: Option<bool>,
    pub encryption_key: Option<String>,
    pub encryption_key_file: Option<String>,
    pub extension_schemas: Option<Vec<String>>,
    pub otlp_endpoint: Option<String>,
    pub idle_connection_timeout: Option<u64>,
    pub drain_timeout: Option<u64>,
    pub datafusion_bridge_queue_depth: Option<usize>,
    pub max_active_scans: Option<usize>,
    pub stream_queue_depth: Option<usize>,
    pub max_buffered_rows: Option<usize>,
    pub max_response_bytes: Option<usize>,
    pub slow_operation_threshold_ms: Option<u64>,
}

pub fn load(explicit: Option<&Path>) -> Result<(Option<PathBuf>, ConfigFile), String> {
    let path = explicit.map(PathBuf::from).or_else(|| {
        let default = PathBuf::from("rocklake.toml");
        default.is_file().then_some(default)
    });
    let Some(path) = path else {
        return Ok((None, ConfigFile::default()));
    };
    let contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
    let config =
        toml::from_str(&contents).map_err(|e| format!("invalid config {}: {e}", path.display()))?;
    Ok((Some(path), config))
}

pub fn example() -> &'static str {
    r#"# RockLake v0.56.0 configuration
catalog = "./lake"
bind = "127.0.0.1:5432"
mode = "writer"
max_sessions = 50
metrics_path = "/metrics"
cost_mode = "balanced"
idle_connection_timeout = 60
drain_timeout = 30
datafusion_bridge_queue_depth = 256
max_active_scans = 25
# max_response_bytes = 16777216
slow_operation_threshold_ms = 1000

# For cloud catalogs, use environment/provider credentials. Secrets may use files:
# auth_user = "ducklake"
# auth_password_file = "/run/secrets/rocklake-auth-password"
# encryption_key_file = "/run/secrets/rocklake-encryption-key"

# Static multi-catalog routing (omit `catalog` when this is configured):
# [router]
# mode = "static"
# default_catalog = "analytics"
# max_open_catalogs = 64
# catalog_idle_timeout = 300
# [[catalogs]]
# id = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9"
# aliases = ["analytics", "analytics_prod"]
# catalog = "s3://company-rocklake/catalogs/analytics"
# data = "s3://company-data/analytics"
# mode = "read-write"
# credential_provider = "aws-default"

# Managed registry (takes precedence over static routing when configured):
# [registry]
# location = "file:///var/lib/rocklake/registry"
# emergency_read_only = true
# recovery_file = "/etc/rocklake/recovery.toml"

# Multi-principal authentication uses SCRAM verifier strings, never plaintext passwords:
# [[principals]]
# id = "018f4f4d-d520-7d91-b9f0-7018b7b50d13"
# username = "analytics_reader"
# scram_verifier = "v=1,i=4096,s=...,sk=...,sv=..."
# [[grants]]
# principal_id = "018f4f4d-d520-7d91-b9f0-7018b7b50d13"
# catalog_id = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9"
# permissions = ["CONNECT", "READ"]
"#
}

pub fn static_router(config: &ConfigFile) -> Result<Option<rocklake_router::StaticConfig>, String> {
    if config.router.is_none() && config.catalogs.is_empty() {
        return Ok(None);
    }
    rocklake_router::StaticConfig::new(
        config.router.clone().unwrap_or_default(),
        config.catalogs.clone(),
    )
    .map(Some)
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates_static_router_config() {
        let config: ConfigFile = toml::from_str(
            r#"
                [router]
                default_catalog = "main"
                [[catalogs]]
                id = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9"
                aliases = ["main"]
                catalog = "file:///tmp/rocklake-config-catalog"
                data = "file:///tmp/rocklake-config-data"
                credential_provider = "env"
            "#,
        )
        .unwrap();
        let router = static_router(&config).unwrap().unwrap();
        assert_eq!(router.settings.default_catalog.as_deref(), Some("main"));
        assert_eq!(router.catalogs.len(), 1);
    }

    #[test]
    fn parses_managed_registry_config() {
        let config: ConfigFile = toml::from_str(
            r#"
                [registry]
                location = "file:///var/lib/rocklake/registry"
                emergency_read_only = true
                recovery_file = "/etc/rocklake/recovery.toml"
            "#,
        )
        .unwrap();
        let registry = config.registry.unwrap();
        assert!(registry.emergency_read_only);
        assert_eq!(registry.location, "file:///var/lib/rocklake/registry");
    }
}
