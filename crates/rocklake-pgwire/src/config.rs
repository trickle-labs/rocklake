//! Typed `rocklake.toml` configuration for the supported binary.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    pub catalog: Option<String>,
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
    r#"# RockLake v0.51.3 configuration
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
"#
}
