//! `rocklake` — CLI binary with all operational commands.
//!
//! Commands:
//!   serve, gc, excise, checkpoint, export, import, pg-migrate,
//!   rebuild, inspect, verify, repair,
//!   warmup, migrate, corpus, tune,
//!   migrate-from-ducklake, export-catalog,
//!   diagnose, sweep-orphans
//!
//! Run `rocklake --help` or `rocklake <command> --help` for full usage.

mod cli;
mod config;

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use clap::{CommandFactory as _, Parser as _};
use clap_complete::generate;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use rocklake_catalog::metrics::CatalogMetrics;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::server::{run_server_with_mode, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(
                "info"
                    .parse()
                    .unwrap_or_else(|_| tracing_subscriber::filter::LevelFilter::INFO.into()),
            ),
        )
        .init();

    dispatch_clap(cli::Cli::parse()).await
}

/// Dispatch based on a successfully-parsed clap CLI.
async fn dispatch_clap(cli: cli::Cli) -> Result<(), Box<dyn std::error::Error>> {
    use cli::Commands;
    let config_path = cli.config.clone();

    match cli.command {
        Commands::Completions(args) => {
            let mut cmd = cli::Cli::command();
            generate(args.shell, &mut cmd, "rocklake", &mut io::stdout());
        }
        Commands::Serve(args) => cmd_serve(*args, config_path.as_deref()).await?,
        Commands::Doctor(args) => cmd_doctor(args, config_path.as_deref()).await?,
        Commands::Config(command) => cmd_config(command, config_path.as_deref()).await?,
        Commands::Backup(command) => cmd_backup(command).await?,
        Commands::Restore(command) => cmd_restore(command).await?,
        Commands::Gc(command) => cmd_gc(command).await?,
        Commands::Excise(command) => cmd_excise(command).await?,
        Commands::Checkpoint(command) => cmd_checkpoint(command).await?,
        Commands::Export(args) => cmd_export(args).await?,
        Commands::Import(args) => cmd_import(args).await?,
        Commands::PgMigrate(args) => cmd_pg_migrate(args).await?,
        Commands::Rebuild(args) => cmd_rebuild(args).await?,
        Commands::Inspect(command) => cmd_inspect(command).await?,
        Commands::Verify(command) => cmd_verify(command).await?,
        Commands::Repair(args) => cmd_repair(args).await?,
        Commands::Warmup(args) => cmd_warmup(args).await?,
        Commands::Migrate(args) => cmd_migrate(args).await?,
        Commands::Corpus(command) => cmd_corpus(command).await?,
        Commands::Tune(args) => cmd_tune(args).await?,
        Commands::MigrateFromDucklake(args) => cmd_migrate_from_ducklake(args).await?,
        Commands::ExportCatalog(args) => cmd_export_catalog(args).await?,
        Commands::Diagnose(args) => cmd_diagnose(args).await?,
        Commands::SweepOrphans(args) => cmd_sweep_orphans(args).await?,
    }
    Ok(())
}

// ─── serve ─────────────────────────────────────────────────────────────────

async fn cmd_serve(
    args: cli::ServeArgs,
    config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_, file_config) = config::load(config_path)?;
    warn_deprecated_limits(
        file_config.stream_queue_depth.is_some()
            || file_config.max_buffered_rows.is_some()
            || args.stream_queue_depth.is_some()
            || args.max_buffered_rows.is_some()
            || std::env::var_os("ROCKLAKE_STREAM_QUEUE_DEPTH").is_some()
            || std::env::var_os("ROCKLAKE_MAX_BUFFERED_ROWS").is_some(),
    );
    let catalog_url = setting(
        args.catalog.or(args.path),
        "ROCKLAKE_CATALOG",
        file_config.catalog.clone(),
        None,
        "catalog",
    )?
    .ok_or("a catalog path is required (use `serve ./lake` or --catalog)")?;
    let mode = if args.read_only.unwrap_or(false) {
        "reader".to_string()
    } else {
        setting(
            args.mode,
            "ROCKLAKE_MODE",
            file_config.mode.clone(),
            Some("writer".to_string()),
            "mode",
        )?
        .unwrap_or_else(|| "writer".to_string())
    };
    if mode != "writer" && mode != "reader" {
        return Err(format!("invalid mode '{mode}' (expected writer or reader)").into());
    }
    let auth_password = read_secret(
        setting(
            args.auth_password,
            "ROCKLAKE_AUTH_PASSWORD",
            file_config.auth_password.clone(),
            None,
            "auth password",
        )?,
        setting(
            args.auth_password_file,
            "ROCKLAKE_AUTH_PASSWORD_FILE",
            file_config.auth_password_file.clone(),
            None,
            "auth password file",
        )?
        .as_deref(),
        "ROCKLAKE_AUTH_PASSWORD_FILE",
    )?;
    let encryption_key = read_secret(
        setting(
            args.encryption_key,
            "ROCKLAKE_ENCRYPTION_KEY",
            file_config.encryption_key.clone(),
            None,
            "encryption key",
        )?,
        setting(
            args.encryption_key_file,
            "ROCKLAKE_ENCRYPTION_KEY_FILE",
            file_config.encryption_key_file.clone(),
            None,
            "encryption key file",
        )?
        .as_deref(),
        "ROCKLAKE_ENCRYPTION_KEY_FILE",
    )?;
    let bind: SocketAddr = setting(
        args.bind,
        "ROCKLAKE_BIND",
        file_config.bind.clone(),
        Some("127.0.0.1:5432".to_string()),
        "bind address",
    )?
    .expect("bind default")
    .parse()
    .map_err(|e| format!("invalid bind address: {e}"))?;
    let max_sessions = setting(
        args.max_sessions,
        "ROCKLAKE_MAX_SESSIONS",
        file_config.max_sessions,
        Some(50),
        "max sessions",
    )?
    .expect("max sessions default");
    let metrics_path = setting(
        args.metrics_path,
        "ROCKLAKE_METRICS_PATH",
        file_config.metrics_path,
        Some("/metrics".to_string()),
        "metrics path",
    )?
    .expect("metrics path default");
    let tls_required = setting(
        args.tls_required,
        "ROCKLAKE_TLS_REQUIRED",
        file_config.tls_required,
        Some(false),
        "tls required",
    )?
    .expect("tls default");
    let cost_mode = setting(
        args.cost_mode,
        "ROCKLAKE_COST_MODE",
        file_config.cost_mode,
        Some("balanced".to_string()),
        "cost mode",
    )?
    .expect("cost mode default");
    let s3_path_style = setting(
        args.s3_path_style,
        "ROCKLAKE_S3_PATH_STYLE",
        file_config.s3_path_style,
        Some(false),
        "s3 path style",
    )?
    .expect("s3 path style default");
    let extension_schemas = args
        .extension_schemas
        .or_else(|| {
            std::env::var("ROCKLAKE_EXTENSION_SCHEMAS")
                .ok()
                .map(|value| {
                    value
                        .split(',')
                        .map(str::trim)
                        .map(str::to_string)
                        .collect()
                })
        })
        .or(file_config.extension_schemas)
        .filter(|schemas: &Vec<String>| !schemas.is_empty())
        .unwrap_or_else(|| vec!["public".to_string(), "pgtrickle".to_string()]);
    let config = ServeConfig {
        catalog_url,
        bind_addr: bind,
        max_sessions,
        metrics_port: setting(
            args.metrics_port,
            "ROCKLAKE_METRICS_PORT",
            file_config.metrics_port,
            None,
            "metrics port",
        )?,
        metrics_path,
        tls_cert: setting(
            args.tls_cert,
            "ROCKLAKE_TLS_CERT",
            file_config.tls_cert,
            None,
            "tls certificate",
        )?,
        tls_key: setting(
            args.tls_key,
            "ROCKLAKE_TLS_KEY",
            file_config.tls_key,
            None,
            "tls key",
        )?,
        tls_required,
        auth_username: setting(
            args.auth_user,
            "ROCKLAKE_AUTH_USER",
            file_config.auth_user,
            None,
            "auth user",
        )?,
        auth_password,
        mode,
        cost_mode: cost_mode
            .parse()
            .map_err(|e| format!("invalid cost mode: {e}"))?,
        s3_endpoint: setting(
            args.s3_endpoint,
            "ROCKLAKE_S3_ENDPOINT",
            file_config.s3_endpoint,
            None,
            "s3 endpoint",
        )?,
        s3_path_style,
        encryption_key,
        extension_schemas,
        otlp_endpoint: setting(
            args.otlp_endpoint,
            "ROCKLAKE_OTLP_ENDPOINT",
            file_config.otlp_endpoint,
            None,
            "otlp endpoint",
        )?,
        idle_connection_timeout_secs: setting(
            args.idle_connection_timeout,
            "ROCKLAKE_IDLE_CONNECTION_TIMEOUT",
            file_config.idle_connection_timeout,
            Some(60),
            "idle connection timeout",
        )?
        .expect("idle timeout default"),
        drain_timeout_secs: setting(
            args.drain_timeout,
            "ROCKLAKE_DRAIN_TIMEOUT",
            file_config.drain_timeout,
            Some(30),
            "drain timeout",
        )?
        .expect("drain timeout default"),
        datafusion_bridge_queue_depth: setting(
            args.datafusion_bridge_queue_depth,
            "ROCKLAKE_DATAFUSION_BRIDGE_QUEUE_DEPTH",
            file_config.datafusion_bridge_queue_depth,
            Some(256),
            "datafusion bridge queue depth",
        )?
        .expect("queue depth default"),
        max_active_scans: setting(
            args.max_active_scans,
            "ROCKLAKE_MAX_ACTIVE_SCANS",
            file_config.max_active_scans,
            Some(25),
            "max active scans",
        )?
        .expect("active scans default"),
        stream_queue_depth: setting(
            args.stream_queue_depth,
            "ROCKLAKE_STREAM_QUEUE_DEPTH",
            file_config.stream_queue_depth,
            Some(64),
            "stream queue depth",
        )?
        .expect("stream queue default"),
        max_buffered_rows: setting(
            args.max_buffered_rows,
            "ROCKLAKE_MAX_BUFFERED_ROWS",
            file_config.max_buffered_rows,
            Some(1024),
            "max buffered rows",
        )?
        .expect("buffered rows default"),
        max_response_bytes: setting(
            args.max_response_bytes,
            "ROCKLAKE_MAX_RESPONSE_BYTES",
            file_config.max_response_bytes,
            Some(16 * 1024 * 1024),
            "max response bytes",
        )?
        .expect("response bytes default"),
        slow_operation_threshold_ms: setting(
            args.slow_operation_threshold_ms,
            "ROCKLAKE_SLOW_OPERATION_THRESHOLD_MS",
            file_config.slow_operation_threshold_ms,
            Some(1000),
            "slow operation threshold",
        )?
        .expect("slow operation threshold default"),
    };
    if config.max_sessions == 0 {
        return Err("max sessions must be greater than zero".into());
    }
    if config.datafusion_bridge_queue_depth == 0
        || config.max_active_scans == 0
        || config.stream_queue_depth == 0
        || config.max_buffered_rows == 0
        || config.max_response_bytes == 0
        || config.slow_operation_threshold_ms == 0
    {
        return Err("resource limits must be greater than zero".into());
    }
    if config.tls_required && (config.tls_cert.is_none() || config.tls_key.is_none()) {
        return Err("tls-required needs both --tls-cert and --tls-key".into());
    }
    let encryption = config
        .encryption_key
        .as_deref()
        .map(rocklake_catalog::EncryptionConfig::from_hex)
        .transpose()
        .map_err(|e| format!("--encryption-key: {e}"))?;

    // v0.39.0: Initialise OTLP tracing if --otlp-endpoint is set.
    let _telemetry = rocklake_pgwire::telemetry::TelemetryConfig {
        otlp_endpoint: config.otlp_endpoint.clone(),
        service_name: "rocklake".to_string(),
    }
    .init();

    let s3_opts = S3Options {
        endpoint: config.s3_endpoint.clone(),
        path_style: config.s3_path_style,
    };
    let (catalog_path, object_store) =
        resolve_catalog_with_opts_mode(&config.catalog_url, &s3_opts, config.mode != "reader")?;

    let opts = OpenOptions {
        object_store: object_store.clone(),
        path: catalog_path,
        encryption,
    };

    let store = if config.mode == "reader" {
        // Read-only mode: skip the writer-epoch CAS so that any number of
        // reader replicas can open the same catalog concurrently without
        // contending on the epoch key.
        tracing::info!("Opening catalog in read-only mode (no writer epoch)");
        CatalogStore::open_without_epoch(opts)
            .await
            .map_err(|e| format!("Failed to open catalog (read-only): {e}"))?
    } else {
        CatalogStore::open(opts)
            .await
            .map_err(|e| format!("Failed to open catalog: {e}"))?
    };
    let access_mode = if config.mode == "reader" {
        rocklake_pgwire::executor::AccessMode::Reader
    } else {
        rocklake_pgwire::executor::AccessMode::Writer
    };

    print_startup_summary(&config, &store);
    tracing::info!("Catalog opened successfully");
    tracing::info!(
        "Serving mode: {}, cost mode: {:?}, datafusion bridge queue depth: {}",
        config.mode,
        config.cost_mode,
        config.datafusion_bridge_queue_depth,
    );

    let catalog = Arc::new(Mutex::new(store));

    // Start metrics server if port specified
    let metrics = Arc::new(CatalogMetrics::new(config.max_sessions as u64));
    if let Some(metrics_port) = config.metrics_port {
        let m = metrics.clone();
        let mpath = config.metrics_path.clone();
        tokio::spawn(async move {
            if let Err(e) =
                rocklake_catalog::metrics::start_metrics_server(m, metrics_port, &mpath).await
            {
                tracing::error!("Metrics server error: {e}");
            }
        });
    }

    // Background task: sync CDC record-count mismatch counter from rocklake-sql global.
    {
        let m = metrics.clone();
        tokio::spawn(async move {
            loop {
                m.set_cdc_record_count_mismatches(rocklake_sql::cdc_record_count_mismatch_total());
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
    }

    let server_config = ServerConfig {
        bind_addr: config.bind_addr,
        max_sessions: config.max_sessions,
        max_active_scans: config.max_active_scans,
        stream_queue_depth: config.stream_queue_depth,
        max_buffered_rows: config.max_buffered_rows,
        max_response_bytes: config.max_response_bytes,
        slow_operation_threshold: std::time::Duration::from_millis(
            config.slow_operation_threshold_ms,
        ),
        metrics: Some(metrics.clone()),
        tls: rocklake_pgwire::server::TlsConfig {
            cert_path: config.tls_cert,
            key_path: config.tls_key,
            required: config.tls_required,
        },
        auth: rocklake_pgwire::server::AuthConfig {
            username: config.auth_username,
            password: config.auth_password,
            scram_sha256: true,
        },
        extension_schemas: config.extension_schemas.clone(),
        idle_connection_timeout: std::time::Duration::from_secs(
            config.idle_connection_timeout_secs,
        ),
        drain_timeout: std::time::Duration::from_secs(config.drain_timeout_secs),
    };

    run_server_with_mode(server_config, catalog, access_mode).await?;
    Ok(())
}

fn setting<T>(
    cli: Option<T>,
    env_name: &str,
    file: Option<T>,
    default: Option<T>,
    label: &str,
) -> Result<Option<T>, String>
where
    T: std::str::FromStr + Clone,
    T::Err: std::fmt::Display,
{
    if cli.is_some() {
        return Ok(cli);
    }
    if let Ok(value) = std::env::var(env_name) {
        return value
            .parse()
            .map(Some)
            .map_err(|e| format!("invalid {label} in {env_name}: {e}"));
    }
    Ok(file.or(default))
}

fn warn_deprecated_limits(configured: bool) {
    if configured {
        eprintln!(
            "WARNING: stream_queue_depth and max_buffered_rows are accepted for compatibility but have no independent runtime effect"
        );
    }
}

fn print_startup_summary(config: &ServeConfig, _store: &CatalogStore) {
    let tls = config.tls_cert.is_some() && config.tls_key.is_some();
    let auth = config.auth_username.is_some() && config.auth_password.is_some();
    println!("RockLake {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Catalog       {}", config.catalog_url);
    println!("Mode          {}", config.mode);
    println!("DuckLake      1.0");
    println!("Listener      {}", config.bind_addr);
    println!("TLS           {}", if tls { "enabled" } else { "disabled" });
    println!(
        "Authentication {}",
        if auth { "SCRAM-SHA-256" } else { "disabled" }
    );
    println!(
        "Metrics       {}",
        config
            .metrics_port
            .map(|port| format!("enabled on {port}"))
            .unwrap_or_else(|| "disabled".to_string())
    );
    println!("Status        ready");
    println!();
    println!("DuckDB:");
    println!(
        "ATTACH 'ducklake:postgres:host={} port={} dbname=rocklake' AS lake (DATA_PATH 'data');",
        config.bind_addr.ip(),
        config.bind_addr.port()
    );
    if !config.bind_addr.ip().is_loopback() && !tls {
        eprintln!("WARNING: listener is not loopback and TLS is disabled");
    }
    if !config.bind_addr.ip().is_loopback() && !auth {
        eprintln!("WARNING: listener is not loopback and authentication is disabled");
    }
}

fn read_secret(
    value: Option<String>,
    file: Option<&str>,
    source_name: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if let Some(value) = value {
        return Ok(Some(value));
    }
    let Some(path) = file else {
        return Ok(None);
    };
    std::fs::read_to_string(path)
        .map(|secret| Some(secret.trim_end_matches(['\r', '\n']).to_owned()))
        .map_err(|error| format!("failed to read {source_name} from {path}: {error}").into())
}

#[cfg(test)]
mod tests {
    use super::{read_secret, redacted_config, setting, validate_config};
    use crate::config::ConfigFile;

    #[test]
    fn read_secret_prefers_value_and_trims_file_newlines() {
        let file = tempfile::NamedTempFile::new().expect("create secret file");
        std::fs::write(file.path(), "from-file\n").expect("write secret file");
        assert_eq!(
            read_secret(None, Some(file.path().to_str().unwrap()), "TEST_SECRET")
                .expect("read secret"),
            Some("from-file".to_string())
        );
        assert_eq!(
            read_secret(
                Some("from-value".to_string()),
                Some("missing"),
                "TEST_SECRET"
            )
            .expect("prefer value"),
            Some("from-value".to_string())
        );
    }

    #[test]
    fn settings_prefer_cli_over_file() {
        assert_eq!(
            setting(
                Some("cli".to_string()),
                "ROCKLAKE_TEST_SETTING_UNSET",
                Some("file".to_string()),
                Some("default".to_string()),
                "test setting",
            )
            .unwrap(),
            Some("cli".to_string())
        );
    }

    #[test]
    fn config_validation_and_redaction_cover_secrets() {
        let config = ConfigFile {
            auth_password: Some("secret".to_string()),
            encryption_key: Some("not-a-key".to_string()),
            ..ConfigFile::default()
        };
        assert!(validate_config(&config).is_err());
        assert!(!redacted_config(&config).to_string().contains("secret"));
    }

    #[test]
    fn config_example_omits_inert_limits() {
        let example = crate::config::example();
        assert!(!example.contains("stream_queue_depth"));
        assert!(!example.contains("max_buffered_rows"));
    }
}

struct ServeConfig {
    catalog_url: String,
    bind_addr: SocketAddr,
    max_sessions: usize,
    metrics_port: Option<u16>,
    /// HTTP path for the metrics endpoint. Default: `/metrics`.
    metrics_path: String,
    tls_cert: Option<String>,
    tls_key: Option<String>,
    tls_required: bool,
    auth_username: Option<String>,
    auth_password: Option<String>,
    /// Serving mode: "writer" (accepts writes) or "reader" (read-only, returns 25006 on writes).
    mode: String,
    /// Cost/latency preset: "conservative", "balanced" (default), or "latency".
    cost_mode: rocklake_catalog::CostMode,
    /// Optional S3-compatible endpoint URL (e.g. for MinIO).
    s3_endpoint: Option<String>,
    /// Use S3 path-style addressing (required for some S3-compatible stores).
    s3_path_style: bool,
    /// Optional AES-256 encryption key (64 hex digits).
    encryption_key: Option<String>,
    /// Allowed extension schema names (default: ["pgtrickle"]).
    extension_schemas: Vec<String>,
    /// Optional OTLP HTTP endpoint for OpenTelemetry tracing (e.g. "http://jaeger:4318").
    /// When not set, no spans are exported. Document: docs/operations/monitoring.md.
    otlp_endpoint: Option<String>,
    /// Duration in seconds after which an idle connection is closed (default: 60).
    idle_connection_timeout_secs: u64,
    /// Grace period in seconds for in-flight queries on SIGTERM drain (default: 30).
    drain_timeout_secs: u64,
    /// Capacity of the DataFusion AsyncBridge channel (default: 256).
    datafusion_bridge_queue_depth: usize,
    max_active_scans: usize,
    stream_queue_depth: usize,
    max_buffered_rows: usize,
    max_response_bytes: usize,
    slow_operation_threshold_ms: u64,
}

// ─── gc ────────────────────────────────────────────────────────────────────

async fn cmd_gc(command: cli::GcSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_url, retention_days, apply, output) = match command {
        cli::GcSubcommand::Plan(args) => (args.catalog, args.retention_days, false, args.output),
        cli::GcSubcommand::Apply(args) => (args.catalog, args.retention_days, true, args.output),
    };
    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    if !apply {
        let plan = rocklake_catalog::gc::gc_plan(&db, retention_days).await?;
        match output {
            cli::OutputFormat::Json => println!(
                "{}",
                serde_json::json!({
                    "schema_version": 1,
                    "current_retain_from": plan.current_retain_from,
                    "proposed_retain_from": plan.proposed_retain_from,
                    "snapshots_affected": plan.snapshots_affected,
                    "pinned_snapshots": plan.pinned_snapshots,
                    "leased_snapshots": plan.leased_snapshots
                })
            ),
            cli::OutputFormat::Human => {
                println!("GC Plan:");
                println!("  Current retain-from: {}", plan.current_retain_from);
                println!("  Proposed retain-from: {}", plan.proposed_retain_from);
                println!("  Snapshots affected: {}", plan.snapshots_affected);
                if !plan.pinned_snapshots.is_empty() {
                    println!("  Pinned snapshots: {:?}", plan.pinned_snapshots);
                }
                if !plan.leased_snapshots.is_empty() {
                    println!("  Leased snapshots: {:?}", plan.leased_snapshots);
                }
            }
        }
    } else {
        let plan = rocklake_catalog::gc::gc_plan(&db, retention_days).await?;
        let result = rocklake_catalog::gc::gc_apply(&db, plan.proposed_retain_from).await?;
        match output {
            cli::OutputFormat::Json => println!(
                "{}",
                serde_json::json!({
                    "schema_version": 1,
                    "previous_retain_from": result.previous_retain_from,
                    "new_retain_from": result.new_retain_from,
                    "snapshots_hidden": result.snapshots_hidden
                })
            ),
            cli::OutputFormat::Human => {
                println!("GC Applied:");
                println!("  Previous retain-from: {}", result.previous_retain_from);
                println!("  New retain-from: {}", result.new_retain_from);
                println!("  Snapshots hidden: {}", result.snapshots_hidden);
            }
        }
    }

    db.close().await?;
    Ok(())
}

// ─── excise ────────────────────────────────────────────────────────────────

async fn cmd_excise(command: cli::ExciseSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_url, before, apply, output) = match command {
        cli::ExciseSubcommand::Plan(args) => (args.catalog, args.before, false, args.output),
        cli::ExciseSubcommand::Apply(args) => (args.catalog, args.before, true, args.output),
    };
    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    if !apply {
        let plan = rocklake_catalog::excise::excise_plan(&db, before).await?;
        match output {
            cli::OutputFormat::Json => println!(
                "{}",
                serde_json::json!({
                    "schema_version": 1,
                    "before_snapshot": plan.before_snapshot,
                    "version_rows_eligible": plan.version_rows_eligible,
                    "inlined_inserts_eligible": plan.inlined_inserts_eligible,
                    "inlined_deletes_eligible": plan.inlined_deletes_eligible,
                    "data_files_eligible": plan.data_files_eligible.len(),
                    "safe": plan.is_safe
                })
            ),
            cli::OutputFormat::Human => {
                println!("Excise Plan:");
                println!("  Before snapshot: {}", plan.before_snapshot);
                println!("  Version rows eligible: {}", plan.version_rows_eligible);
                println!(
                    "  Inlined inserts eligible: {}",
                    plan.inlined_inserts_eligible
                );
                println!(
                    "  Inlined deletes eligible: {}",
                    plan.inlined_deletes_eligible
                );
                println!("  Data files eligible: {}", plan.data_files_eligible.len());
                println!("  Safe: {}", if plan.is_safe { "yes" } else { "NO" });
            }
        }
    } else {
        let result = rocklake_catalog::excise::excise_apply(&db, before, "operator").await?;
        match output {
            cli::OutputFormat::Json => println!(
                "{}",
                serde_json::json!({
                    "schema_version": 1,
                    "keys_deleted": result.keys_deleted,
                    "keys_failed": result.keys_failed,
                    "audit_entry_id": result.audit_entry_id
                })
            ),
            cli::OutputFormat::Human => {
                println!("Excise Applied:");
                println!("  Keys deleted: {}", result.keys_deleted);
                println!("  Keys failed: {}", result.keys_failed);
                println!("  Audit entry ID: {}", result.audit_entry_id);
            }
        }
        if result.keys_failed > 0 {
            return Err(format!(
                "excision incomplete: {} catalog deletions failed",
                result.keys_failed
            )
            .into());
        }
    }

    db.close().await?;
    Ok(())
}

// ─── checkpoint ────────────────────────────────────────────────────────────

async fn cmd_checkpoint(
    command: cli::CheckpointSubcommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let catalog_url = match &command {
        cli::CheckpointSubcommand::Create(args) => &args.catalog,
        cli::CheckpointSubcommand::List(args) => &args.catalog,
        cli::CheckpointSubcommand::Restore(args) => &args.catalog,
        cli::CheckpointSubcommand::Pin(args) => &args.catalog,
        cli::CheckpointSubcommand::Unpin(args) => &args.catalog,
        cli::CheckpointSubcommand::Pins(args) => &args.catalog,
    };
    let (catalog_path, object_store) = resolve_catalog(catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    match command {
        cli::CheckpointSubcommand::Create(args) => {
            let label = args.label;
            let info =
                rocklake_catalog::checkpoint::create_checkpoint(&db, label.as_deref()).await?;
            println!("Checkpoint created:");
            println!("  ID: {}", info.id);
            println!("  Snapshot ID: {}", info.snapshot_id);
            println!("  Created at: {}", info.created_at);
        }
        cli::CheckpointSubcommand::List(_) => {
            let checkpoints = rocklake_catalog::checkpoint::list_checkpoints(&db).await?;
            if checkpoints.is_empty() {
                println!("No checkpoints found.");
            } else {
                println!("{:<20} {:<12} {:<30} Label", "ID", "Snapshot", "Created");
                for cp in checkpoints {
                    println!(
                        "{:<20} {:<12} {:<30} {}",
                        cp.id,
                        cp.snapshot_id,
                        cp.created_at,
                        cp.label.unwrap_or_default()
                    );
                }
            }
        }
        cli::CheckpointSubcommand::Restore(args) => {
            let info = rocklake_catalog::checkpoint::restore_checkpoint(&db, args.id).await?;
            println!("Checkpoint restored:");
            println!("  ID: {}", info.id);
            println!(
                "  Restored to snapshot: {}",
                info.restore_snapshot_id.unwrap_or(info.snapshot_id)
            );
        }
        cli::CheckpointSubcommand::Pin(args) => {
            let pin = rocklake_catalog::checkpoint::pin_checkpoint(&db, &args.name, args.snapshot)
                .await?;
            println!("Checkpoint pin created:");
            println!("  Name: {}", pin.name);
            println!("  Snapshot ID: {}", pin.snapshot_id);
        }
        cli::CheckpointSubcommand::Unpin(args) => {
            rocklake_catalog::checkpoint::unpin_checkpoint(&db, &args.name).await?;
            println!("Checkpoint pin removed: {}", args.name);
        }
        cli::CheckpointSubcommand::Pins(_) => {
            let pins = rocklake_catalog::checkpoint::list_checkpoint_pins(&db).await?;
            for pin in pins {
                println!("{} {} {}", pin.name, pin.snapshot_id, pin.created_at);
            }
        }
    }

    db.close().await?;
    Ok(())
}

// ─── export ────────────────────────────────────────────────────────────────

async fn cmd_export(args: cli::ExportArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let output_path = args.output;
    let snapshot_id = args.snapshot_id;

    let mut file = std::fs::File::create(&output_path)
        .map_err(|e| format!("Cannot create output file: {e}"))?;

    let result = rocklake_catalog::export::export_catalog(&db, snapshot_id, &mut file).await?;
    println!("Export complete:");
    println!("  Rows exported: {}", result.rows_exported);
    println!("  Tables exported: {}", result.tables_exported);
    println!("  Output: {output_path}");

    db.close().await?;
    Ok(())
}

// ─── import ────────────────────────────────────────────────────────────────

async fn cmd_import(args: cli::ImportArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let input_path = args.input;

    let file =
        std::fs::File::open(&input_path).map_err(|e| format!("Cannot open input file: {e}"))?;
    let reader = std::io::BufReader::new(file);

    let result = rocklake_catalog::export::import_catalog(&db, reader).await?;
    println!("Import complete:");
    println!("  Rows imported: {}", result.rows_imported);
    println!("  Tables imported: {}", result.tables_imported);

    db.close().await?;
    Ok(())
}

// ─── pg-migrate ────────────────────────────────────────────────────────────

async fn cmd_pg_migrate(args: cli::PgMigrateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let input_path = args.input;

    let file =
        std::fs::File::open(&input_path).map_err(|e| format!("Cannot open input file: {e}"))?;
    let reader = std::io::BufReader::new(file);

    let mut stdout = std::io::stdout();
    let count = rocklake_catalog::export::pg_migrate(reader, &mut stdout)?;
    eprintln!("Generated {count} INSERT statements.");

    Ok(())
}

// ─── rebuild ───────────────────────────────────────────────────────────────

async fn cmd_rebuild(args: cli::RebuildArgs) -> Result<(), Box<dyn std::error::Error>> {
    let data_path = args
        .data_root
        .ok_or("--data-root is required for rebuild")?;
    let s3_opts = S3Options {
        endpoint: args.s3_endpoint,
        path_style: args.s3_path_style,
    };
    let (catalog_path, object_store) = resolve_catalog_with_opts(&args.catalog, &s3_opts)?;
    let db = slatedb::Db::open(catalog_path, object_store.clone()).await?;

    // List Parquet files in the data path
    let data_prefix = ObjectPath::from(data_path.as_str());
    let mut data_paths = Vec::new();

    use futures::TryStreamExt;
    let objects: Vec<_> = object_store
        .list(Some(&data_prefix))
        .try_collect()
        .await
        .map_err(|e| format!("Failed to list objects at '{data_path}': {e}"))?;

    for obj in objects {
        let path_str = obj.location.to_string();
        if path_str.ends_with(".parquet") {
            data_paths.push(path_str);
        }
    }

    let count = rocklake_catalog::export::rebuild_catalog(&db, &data_paths).await?;
    println!("Rebuild complete: {count} files registered.");

    db.close().await?;
    Ok(())
}

// ─── inspect ───────────────────────────────────────────────────────────────

async fn cmd_inspect(command: cli::InspectSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        cli::InspectSubcommand::Snapshot(args) => {
            let output = args.output;
            let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
            let db = slatedb::Db::open(catalog_path, object_store).await?;

            let result = rocklake_catalog::inspect::inspect_snapshot(&db).await?;
            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "latest_snapshot_id": result.latest_snapshot_id,
                        "schema_version_id": result.schema_version,
                        "snapshot_time": result.snapshot_time,
                        "next_snapshot_id": result.next_snapshot_id,
                        "next_catalog_id": result.next_catalog_id,
                        "next_file_id": result.next_file_id,
                        "schema_count": result.schema_count,
                        "table_count": result.table_count,
                        "column_count": result.column_count,
                        "data_file_count": result.data_file_count,
                        "delete_file_count": result.delete_file_count,
                        "retain_from": result.retain_from,
                        "writer_epoch": result.writer_epoch,
                        "format_version": result.format_version
                    })
                ),
                cli::OutputFormat::Human => {
                    println!("Catalog State:");
                    println!("  Latest snapshot ID: {}", result.latest_snapshot_id);
                    println!("  Schema version: {}", result.schema_version);
                    println!("  Snapshot time: {}", result.snapshot_time);
                    println!("  Next snapshot ID: {}", result.next_snapshot_id);
                    println!("  Next catalog ID: {}", result.next_catalog_id);
                    println!("  Next file ID: {}", result.next_file_id);
                    println!("  Schemas: {}", result.schema_count);
                    println!("  Tables: {}", result.table_count);
                    println!("  Columns: {}", result.column_count);
                    println!("  Data files: {}", result.data_file_count);
                    println!("  Delete files: {}", result.delete_file_count);
                    println!("  Retain-from: {}", result.retain_from);
                    println!("  Writer epoch: {}", result.writer_epoch);
                    println!("  Format version: {}", result.format_version);
                }
            }

            db.close().await?;
        }
        cli::InspectSubcommand::ApiCosts(args) => {
            let output = args.output;
            let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
            let db = slatedb::Db::open(catalog_path, object_store).await?;
            let state = rocklake_catalog::inspect::inspect_snapshot(&db).await?;
            db.close().await?;

            let file_count = state.data_file_count;
            let snap = rocklake_catalog::cost::ApiCallSnapshot {
                put_count: file_count * 3,
                get_count: file_count * 10,
                list_count: file_count / 10 + 1,
                delete_count: 0,
                elapsed: std::time::Duration::from_secs(3600),
            };
            let report = rocklake_catalog::cost::ApiCostReport::from_snapshot(&snap);

            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "put_count": report.put_count,
                        "get_count": report.get_count,
                        "list_count": report.list_count,
                        "delete_count": report.delete_count,
                        "elapsed_secs": report.elapsed_secs,
                        "estimated_monthly_usd": report.estimated_monthly_usd,
                        "rds_monthly_usd": report.rds_monthly_usd,
                        "put_per_minute": report.put_per_minute,
                        "get_per_minute": report.get_per_minute,
                        "list_per_minute": report.list_per_minute,
                        "recommendations": report.recommendations
                    })
                ),
                cli::OutputFormat::Human => report.print(),
            }
        }
        cli::InspectSubcommand::CacheUtilization(args) => {
            let output = args.output;
            let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
            let db = slatedb::Db::open(catalog_path, object_store).await?;
            let state = rocklake_catalog::inspect::inspect_snapshot(&db).await?;
            db.close().await?;

            let stats =
                rocklake_catalog::cache_utilization(256, state.data_file_count, state.column_count)
                    .await;
            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "hits": stats.hits,
                        "misses": stats.misses,
                        "hit_ratio": stats.hit_ratio,
                        "evictions": stats.evictions,
                        "bytes_used": stats.bytes_used,
                        "capacity_bytes": stats.capacity_bytes,
                        "recommended_cache_size_mb": stats.recommended_cache_size_mb
                    })
                ),
                cli::OutputFormat::Human => stats.print(),
            }
        }
    }

    Ok(())
}

// ─── verify ────────────────────────────────────────────────────────────────

async fn cmd_verify(command: cli::VerifySubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let catalog_url = match &command {
        cli::VerifySubcommand::Catalog(args) => &args.catalog,
        cli::VerifySubcommand::DataFiles(args) => &args.catalog,
    };
    let output = match &command {
        cli::VerifySubcommand::Catalog(args) => args.output,
        cli::VerifySubcommand::DataFiles(args) => args.output,
    };
    let (catalog_path, object_store) = resolve_catalog(catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store.clone()).await?;

    match command {
        cli::VerifySubcommand::Catalog(_) => {
            let result = rocklake_catalog::verify::verify_catalog(&db).await?;
            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "tables_checked": result.tables_checked,
                        "rows_checked": result.rows_checked,
                        "errors": result.errors,
                        "warnings": result.warnings,
                        "ok": result.is_ok()
                    })
                ),
                cli::OutputFormat::Human => {
                    println!("Catalog Verification:");
                    println!("  Tables checked: {}", result.tables_checked);
                    println!("  Rows checked: {}", result.rows_checked);
                    if result.errors.is_empty() {
                        println!("  Status: OK");
                    } else {
                        println!("  Errors:");
                        for err in &result.errors {
                            println!("    - {err}");
                        }
                    }
                    if !result.warnings.is_empty() {
                        println!("  Warnings:");
                        for warn in &result.warnings {
                            println!("    - {warn}");
                        }
                    }
                }
            }
        }
        cli::VerifySubcommand::DataFiles(_) => {
            let result = rocklake_catalog::cleanup::verify_data_files(&db, &object_store).await?;
            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "files_ok": result.files_ok,
                        "files_missing": result.files_missing,
                        "files_error": result.files_error,
                        "total_checked": result.total_checked
                    })
                ),
                cli::OutputFormat::Human => {
                    println!("Data File Verification:");
                    println!("  Files OK: {}", result.files_ok);
                    println!("  Files missing: {}", result.files_missing.len());
                    println!("  Files error: {}", result.files_error.len());
                    println!("  Total checked: {}", result.total_checked);
                    if !result.files_missing.is_empty() {
                        println!("  Missing files:");
                        for path in &result.files_missing {
                            println!("    - {path}");
                        }
                    }
                }
            }
        }
    }

    db.close().await?;
    Ok(())
}

// ─── repair ────────────────────────────────────────────────────────────────

async fn cmd_repair(args: cli::RepairArgs) -> Result<(), Box<dyn std::error::Error>> {
    let output = args.output;
    let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let plan = rocklake_catalog::repair::repair_plan(&db).await?;

    if plan.is_empty() {
        match output {
            cli::OutputFormat::Json => println!(
                "{}",
                serde_json::json!({"schema_version": 1, "actions": [], "unrecoverable_errors": [], "applied": false})
            ),
            cli::OutputFormat::Human => println!("No repairs needed. Catalog is healthy."),
        }
    } else {
        if matches!(output, cli::OutputFormat::Json) && !args.apply {
            println!(
                "{}",
                serde_json::json!({
                    "schema_version": 1,
                    "actions": plan.actions.iter().map(|action| format!("{action:?}")).collect::<Vec<_>>(),
                    "unrecoverable_errors": &plan.unrecoverable_errors,
                    "applied": args.apply
                })
            );
        } else if matches!(output, cli::OutputFormat::Human) {
            println!("Repair Plan:");
            for action in &plan.actions {
                println!("  - {action:?}");
            }
            if plan.has_unrecoverable() {
                println!("  UNRECOVERABLE ERRORS (restore from backup):");
                for err in &plan.unrecoverable_errors {
                    println!("    - {err}");
                }
            }
        }

        if args.apply && !plan.has_unrecoverable() {
            let result = rocklake_catalog::repair::repair_apply(&db, &plan).await?;
            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "actions": plan.actions.iter().map(|action| format!("{action:?}")).collect::<Vec<_>>(),
                        "unrecoverable_errors": plan.unrecoverable_errors,
                        "applied": true,
                        "actions_applied": result.actions_applied,
                        "actions_failed": result.actions_failed
                    })
                ),
                cli::OutputFormat::Human => {
                    println!("Repair Applied:");
                    println!("  Actions applied: {}", result.actions_applied);
                    println!("  Actions failed: {}", result.actions_failed);
                }
            }
        } else if !args.apply && matches!(output, cli::OutputFormat::Human) {
            println!("\nDry run. Use --apply to execute repairs.");
        }
    }

    db.close().await?;
    Ok(())
}

// ─── warmup ────────────────────────────────────────────────────────────────

async fn cmd_warmup(args: cli::WarmupArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let max_tables = args.tables.unwrap_or(20) as usize;
    let result = rocklake_catalog::warmup_cache(&db, max_tables).await?;

    println!("Cache Warmup Complete:");
    println!("  Entries warmed:   {}", result.entries_warmed);
    println!("  Snapshot loaded:  {}", result.snapshot_loaded);
    println!("  Warmup hit ratio: {:.2}", result.warmup_hit_ratio);

    if result.warmup_hit_ratio >= 0.5 {
        println!("  Status: OK — cache warm for first requests");
    } else {
        println!("  Status: COLD — first requests will pay S3 round-trip latency");
    }

    db.close().await?;
    Ok(())
}

// ─── migrate ───────────────────────────────────────────────────────────────

async fn cmd_migrate(args: cli::MigrateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let target_version = 2;
    let apply = args.apply;
    let dry_run = args.dry_run || !apply;

    if dry_run {
        let result = rocklake_catalog::migrate::migrate_dry_run(&db, target_version).await?;
        println!("Migration Dry Run:");
        println!("  Current version:    {}", result.current_version);
        println!("  Target version:     {}", result.target_version);
        println!("  Rows to migrate:    {}", result.rows_to_migrate);
        println!("  Estimated duration: ~{}s", result.estimated_seconds);
        println!();
        println!("{}", result.description);
        if result.rows_to_migrate > 0 {
            println!();
            println!("Run with --apply to execute the migration.");
        }
    } else {
        let backup_dir = ".";
        let result =
            rocklake_catalog::migrate::migrate_apply(&db, target_version, backup_dir).await?;
        println!("Migration Complete:");
        println!("  Rows migrated:  {}", result.rows_migrated);
        println!("  New version:    {}", result.new_version);
        println!("  Backup written: {}", result.backup_path);
    }

    db.close().await?;
    Ok(())
}

// ─── corpus ────────────────────────────────────────────────────────────────

async fn cmd_corpus(command: cli::CorpusSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        cli::CorpusSubcommand::Diff(args) => {
            let old_path = args.left;
            let new_path = args.right;
            let old_file = std::fs::File::open(&old_path)
                .map_err(|e| format!("Cannot open old corpus: {e}"))?;
            let new_file = std::fs::File::open(&new_path)
                .map_err(|e| format!("Cannot open new corpus: {e}"))?;

            let old_records = rocklake_catalog::parse_corpus(std::io::BufReader::new(old_file));
            let new_records = rocklake_catalog::parse_corpus(std::io::BufReader::new(new_file));
            let diffs = rocklake_catalog::corpus_diff(&old_records, &new_records);

            if diffs.is_empty() {
                println!("No differences found between corpus files.");
            } else {
                println!("Corpus Diff ({} changes):", diffs.len());
                for d in &diffs {
                    println!(
                        "  [{:8}] {} — {}",
                        d.change_type, d.statement_family, d.detail
                    );
                }
            }
        }
        cli::CorpusSubcommand::Validate(args) => {
            let corpus_path = args.corpus;
            let path = std::path::Path::new(&corpus_path);
            let mut all_records = Vec::new();
            if path.is_dir() {
                let mut entries: Vec<_> = std::fs::read_dir(path)
                    .map_err(|e| format!("Cannot read corpus directory: {e}"))?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
                    .collect();
                entries.sort_by_key(|e| e.file_name());
                for entry in entries {
                    let file = std::fs::File::open(entry.path())
                        .map_err(|e| format!("Cannot open corpus file: {e}"))?;
                    let mut records = rocklake_catalog::parse_corpus(std::io::BufReader::new(file));
                    all_records.append(&mut records);
                }
            } else {
                let file =
                    std::fs::File::open(path).map_err(|e| format!("Cannot open corpus: {e}"))?;
                all_records = rocklake_catalog::parse_corpus(std::io::BufReader::new(file));
            }
            let result = rocklake_catalog::corpus_validate(&all_records);
            result.print();
        }
    }

    Ok(())
}

// ─── tune ──────────────────────────────────────────────────────────────────

async fn cmd_tune(args: cli::TuneArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;
    let state = rocklake_catalog::inspect::inspect_snapshot(&db).await?;
    db.close().await?;

    let target_cost = args.target_cost_usd.unwrap_or(50.0);

    // Build a cost report from catalog metadata
    let snap = rocklake_catalog::cost::ApiCallSnapshot {
        put_count: state.data_file_count * 3,
        get_count: state.data_file_count * 10,
        list_count: state.data_file_count / 10 + 1,
        delete_count: 0,
        elapsed: std::time::Duration::from_secs(3600),
    };
    let report = rocklake_catalog::cost::ApiCostReport::from_snapshot(&snap);

    println!("RockLake Tuning Recommendations");
    println!("=================================");
    println!("Target monthly cost: ${target_cost:.2}");
    println!();

    let recs = rocklake_catalog::tune_for_cost_target(target_cost, &report);
    for r in &recs {
        println!("{r}");
    }

    println!();
    println!("Cost Mode Profiles:");
    for mode in [
        rocklake_catalog::CostMode::Conservative,
        rocklake_catalog::CostMode::Balanced,
        rocklake_catalog::CostMode::Latency,
    ] {
        let name = match mode {
            rocklake_catalog::CostMode::Conservative => "conservative",
            rocklake_catalog::CostMode::Balanced => "balanced",
            rocklake_catalog::CostMode::Latency => "latency",
        };
        println!("  --cost-mode={name}");
        println!("    {}", mode.profile_description());
    }

    Ok(())
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Options for S3-compatible object store configuration.
#[derive(Default)]
struct S3Options {
    endpoint: Option<String>,
    path_style: bool,
}

fn resolve_catalog(url: &str) -> Result<(ObjectPath, Arc<dyn object_store::ObjectStore>), String> {
    resolve_catalog_with_opts(url, &S3Options::default())
}

fn resolve_catalog_with_opts(
    url: &str,
    s3_opts: &S3Options,
) -> Result<(ObjectPath, Arc<dyn object_store::ObjectStore>), String> {
    resolve_catalog_with_opts_mode(url, s3_opts, true)
}

fn resolve_catalog_with_opts_mode(
    url: &str,
    s3_opts: &S3Options,
    create_local_root: bool,
) -> Result<(ObjectPath, Arc<dyn object_store::ObjectStore>), String> {
    let url = url.strip_prefix("file://").unwrap_or(url);
    if let Some(without_scheme) = url.strip_prefix("s3://") {
        let (bucket, prefix) = match without_scheme.find('/') {
            Some(idx) => (&without_scheme[..idx], &without_scheme[idx + 1..]),
            None => (without_scheme, ""),
        };
        rocklake_core::path::validate_object_prefix(prefix)
            .map_err(|e| format!("invalid catalog prefix: {e}"))?;

        let mut builder = object_store::aws::AmazonS3Builder::from_env().with_bucket_name(bucket);
        if let Some(ref endpoint) = s3_opts.endpoint {
            builder = builder.with_endpoint(endpoint);
        }
        if s3_opts.path_style {
            builder = builder.with_virtual_hosted_style_request(false);
        }
        let store = builder
            .build()
            .map_err(|e| format!("Failed to create S3 object store: {e}"))?;

        let obj_path = ObjectPath::from(prefix);
        Ok((obj_path, Arc::new(store)))
    } else if let Some(without_scheme) = url.strip_prefix("gs://") {
        let (bucket, prefix) = match without_scheme.find('/') {
            Some(idx) => (&without_scheme[..idx], &without_scheme[idx + 1..]),
            None => (without_scheme, ""),
        };
        rocklake_core::path::validate_object_prefix(prefix)
            .map_err(|e| format!("invalid catalog prefix: {e}"))?;

        let store = object_store::gcp::GoogleCloudStorageBuilder::from_env()
            .with_bucket_name(bucket)
            .build()
            .map_err(|e| format!("Failed to create GCS object store: {e}"))?;

        let obj_path = ObjectPath::from(prefix);
        Ok((obj_path, Arc::new(store)))
    } else if let Some(without_scheme) = url
        .strip_prefix("az://")
        .or_else(|| url.strip_prefix("azure://"))
        .or_else(|| url.strip_prefix("abfs://"))
        .or_else(|| url.strip_prefix("abfss://"))
    {
        let (container, prefix) = match without_scheme.find('/') {
            Some(idx) => (&without_scheme[..idx], &without_scheme[idx + 1..]),
            None => (without_scheme, ""),
        };
        rocklake_core::path::validate_object_prefix(prefix)
            .map_err(|e| format!("invalid catalog prefix: {e}"))?;

        let store = object_store::azure::MicrosoftAzureBuilder::from_env()
            .with_container_name(container)
            .build()
            .map_err(|e| format!("Failed to create Azure object store: {e}"))?;

        let obj_path = ObjectPath::from(prefix);
        Ok((obj_path, Arc::new(store)))
    } else {
        if url.contains("://") {
            return Err(format!("unsupported catalog URI scheme in '{url}'"));
        }
        let path = std::path::Path::new(url);
        let canonical = if path.exists() {
            path.canonicalize()
                .map_err(|e| format!("cannot resolve path: {e}"))?
        } else {
            if !create_local_root {
                return Err(format!(
                    "catalog path '{url}' does not exist; writer initialization is required"
                ));
            }
            std::fs::create_dir_all(path).map_err(|e| format!("cannot create catalog dir: {e}"))?;
            path.canonicalize()
                .map_err(|e| format!("cannot resolve path: {e}"))?
        };

        let store = Arc::new(
            LocalFileSystem::new_with_prefix(&canonical)
                .map_err(|e| format!("cannot create local object store: {e}"))?,
        );
        let obj_path = ObjectPath::from("");

        Ok((obj_path, store))
    }
}

// ─── migrate-from-ducklake ─────────────────────────────────────────────────

/// Import an existing DuckLake catalog into RockLake.
///
/// The source can be:
///   - A SQLite DuckLake catalog:  `--source sqlite:/path/to/catalog.db`
///   - A PostgreSQL DuckLake catalog: `--source postgres://...`
///   - An NDJSON dump (legacy):    `--source /path/to/dump.ndjson`
///
/// Use `--accept-version V1_1_DEV_1` to allow migration from a DuckLake v1.1
/// pre-release catalog (catalog_version 8).  By default only v1.0 (version 7)
/// is accepted.
///
/// Use `--dry-run` to inspect the migration plan without writing anything.
///
/// Example:
///   rocklake migrate-from-ducklake --source sqlite:./duck.db --catalog ./my-catalog
///   rocklake migrate-from-ducklake --source dump.ndjson --catalog ./my-catalog
async fn cmd_migrate_from_ducklake(
    args: cli::MigrateFromDucklakeArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = args.source;
    let catalog_url = args.catalog;
    let dry_run = args.dry_run;
    let accept_refs: Vec<&str> = args.accept_versions.iter().map(String::as_str).collect();

    println!("migrate-from-ducklake: source={source}, catalog={catalog_url}, dry_run={dry_run}");

    // Open the destination RockLake catalog.
    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    if let Some(sqlite_path) = source.strip_prefix("sqlite:") {
        // ── SQLite DuckLake source ──────────────────────────────────────────
        let mut src =
            rocklake_catalog::migrate_from_ducklake::SqliteDuckLakeSource::open(sqlite_path, None)?;
        let report = rocklake_catalog::migrate_from_ducklake::migrate_from_source(
            &mut src,
            &db,
            &accept_refs,
            dry_run,
        )
        .await?;

        println!(
            "Migration {}:",
            if dry_run { "dry-run" } else { "complete" }
        );
        println!(
            "  Source catalog version: {}",
            report.source_catalog_version
        );
        println!("  Source snapshot:       {}", report.source_snapshot_id);
        println!("  Data files:      {}", report.data_file_count);
        println!("  Total migrated:  {}", report.total_migrated());
        println!("  Total skipped:   {}", report.total_skipped());
        if !dry_run {
            println!("  Catalog written to: {catalog_url}");
        }
    } else if source.starts_with("postgres://") || source.starts_with("postgresql://") {
        let mut src =
            rocklake_catalog::migrate_from_ducklake::PostgresDuckLakeSource::connect(&source, None)
                .await?;
        let report = rocklake_catalog::migrate_from_ducklake::migrate_from_source(
            &mut src,
            &db,
            &accept_refs,
            dry_run,
        )
        .await?;

        println!(
            "Migration {}:",
            if dry_run { "dry-run" } else { "complete" }
        );
        println!("  Source snapshot:       {}", report.source_snapshot_id);
        println!(
            "  Source catalog version: {}",
            report.source_catalog_version
        );
        println!("  Data files:      {}", report.data_file_count);
        println!("  Total migrated:  {}", report.total_migrated());
        println!("  Total skipped:   {}", report.total_skipped());
        if !dry_run {
            println!("  Catalog written to: {catalog_url}");
        }
    } else {
        // ── NDJSON dump source (legacy) ─────────────────────────────────────
        let file =
            std::fs::File::open(&source).map_err(|e| format!("Cannot open source file: {e}"))?;
        let reader = std::io::BufReader::new(file);

        let result = rocklake_catalog::export::import_catalog(&db, reader).await?;

        println!("Migration complete (NDJSON source):");
        println!("  Rows imported:   {}", result.rows_imported);
        println!("  Tables imported: {}", result.tables_imported);
        println!("  Catalog written to: {catalog_url}");
    }

    db.close().await?;
    Ok(())
}

// ─── export-catalog ────────────────────────────────────────────────────────

/// Export all DuckLake catalog tables (28 spec + 4 extension) to a JSON-lines file.
///
/// This produces an interop dump suitable for migration or debugging.
/// Sensitive fields (encryption keys, secrets) are redacted in the output.
///
/// Example:
///   rocklake export-catalog --catalog ./my-catalog --out catalog-dump.ndjson
///   rocklake export-catalog --catalog ./my-catalog --out snap1.ndjson --at-snapshot 1
async fn cmd_export_catalog(
    args: cli::ExportCatalogArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let catalog_url = args.catalog;
    let output_path = args.out;
    let snapshot_id = args.at_snapshot;

    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let mut file = std::fs::File::create(&output_path)
        .map_err(|e| format!("Cannot create output file {output_path}: {e}"))?;

    let result = rocklake_catalog::export::export_catalog(&db, snapshot_id, &mut file).await?;

    println!("Export complete (28 DuckLake spec + 4 extension catalog tables):");
    println!("  Rows exported:   {}", result.rows_exported);
    println!("  Tables exported: {}", result.tables_exported);
    println!("  Output:          {output_path}");

    db.close().await?;
    Ok(())
}

// ─── diagnose (v0.39.0) ────────────────────────────────────────────────────

/// Run a structured health diagnostic against a catalog.
///
/// Example:
///   rocklake diagnose --catalog ./my-catalog
///   rocklake diagnose --catalog s3://bucket/catalog/ --json
///   rocklake diagnose --catalog ./my-catalog --data-root ./data/
async fn cmd_diagnose(args: cli::DiagnoseArgs) -> Result<(), Box<dyn std::error::Error>> {
    let catalog_url = args.catalog;
    let json_output = args.json || matches!(args.output, cli::OutputFormat::Json);
    let data_root = args.data_root;

    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store.clone()).await?;

    let store_and_root = data_root.map(|root| (object_store, root));

    let report = rocklake_catalog::diagnose_catalog(&db, store_and_root).await?;
    db.close().await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", rocklake_catalog::format_report_text(&report));
    }

    // Exit non-zero if P0 findings are present (suitable for CI gates).
    if !report.is_ok() {
        std::process::exit(1);
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct DoctorCheck {
    name: String,
    status: String,
    message: String,
}

#[derive(Debug, serde::Serialize)]
struct DoctorReport {
    schema_version: u32,
    rocklake_version: &'static str,
    catalog: String,
    mode: String,
    ready: bool,
    checks: Vec<DoctorCheck>,
    warnings: Vec<String>,
}

fn doctor_check(
    checks: &mut Vec<DoctorCheck>,
    name: &str,
    status: &str,
    message: impl Into<String>,
) {
    checks.push(DoctorCheck {
        name: name.to_string(),
        status: status.to_string(),
        message: message.into(),
    });
}

async fn cmd_doctor(
    args: cli::DoctorArgs,
    config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    use futures::TryStreamExt;
    use object_store::ObjectStore;

    let (_, file_config) = config::load(config_path)?;
    let mode = setting(
        args.mode,
        "ROCKLAKE_MODE",
        file_config.mode.clone(),
        Some("writer".to_string()),
        "mode",
    )?
    .expect("doctor mode default");
    let bind = setting(
        args.bind,
        "ROCKLAKE_BIND",
        file_config.bind.clone(),
        Some("127.0.0.1:5432".to_string()),
        "bind address",
    )?
    .expect("doctor bind default")
    .parse::<SocketAddr>()
    .map_err(|e| format!("invalid bind address: {e}"))?;
    let mut checks = Vec::new();
    let mut warnings = Vec::new();

    let encryption_key = setting(
        args.encryption_key,
        "ROCKLAKE_ENCRYPTION_KEY",
        file_config.encryption_key.clone(),
        None,
        "encryption key",
    )?;
    let encryption_key_file = setting(
        args.encryption_key_file,
        "ROCKLAKE_ENCRYPTION_KEY_FILE",
        file_config.encryption_key_file.clone(),
        None,
        "encryption key file",
    )?;
    let encryption = read_secret(
        encryption_key,
        encryption_key_file.as_deref(),
        "ROCKLAKE_ENCRYPTION_KEY_FILE",
    )?;
    match encryption.as_deref() {
        Some(key) => match rocklake_catalog::EncryptionConfig::from_hex(key) {
            Ok(_) => doctor_check(&mut checks, "encryption", "pass", "encryption key is valid"),
            Err(error) => doctor_check(&mut checks, "encryption", "fail", error.to_string()),
        },
        None => doctor_check(
            &mut checks,
            "encryption",
            "pass",
            "encryption not configured",
        ),
    }

    let local_path = args
        .catalog
        .strip_prefix("file://")
        .or_else(|| (!args.catalog.contains("://")).then_some(args.catalog.as_str()));
    let mut location = None;
    if let Some(path_text) = local_path {
        doctor_check(&mut checks, "uri", "pass", "valid local catalog path");
        doctor_check(
            &mut checks,
            "credentials",
            "pass",
            "local filesystem needs no credentials",
        );
        doctor_check(
            &mut checks,
            "connectivity",
            "pass",
            "local filesystem is reachable",
        );
        let path = std::path::Path::new(path_text);
        if path.exists() {
            let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let writable = !std::fs::metadata(parent)?.permissions().readonly();
            doctor_check(
                &mut checks,
                "catalog prefix",
                "pass",
                "local catalog directory exists",
            );
            doctor_check(
                &mut checks,
                "read permission",
                "pass",
                "local catalog can be read",
            );
            doctor_check(
                &mut checks,
                "write permission",
                if writable { "pass" } else { "fail" },
                if writable {
                    "parent directory is writable"
                } else {
                    "parent directory is read-only"
                },
            );
            location = Some(resolve_catalog_with_opts_mode(
                &args.catalog,
                &S3Options::default(),
                false,
            ));
        } else {
            let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let parent_ok = parent.is_dir();
            doctor_check(
                &mut checks,
                "catalog prefix",
                "pass",
                "fresh local catalog will be created by serve",
            );
            doctor_check(
                &mut checks,
                "read permission",
                if parent_ok { "pass" } else { "fail" },
                if parent_ok {
                    "parent directory is available"
                } else {
                    "parent directory does not exist"
                },
            );
            let writable = parent_ok
                && std::fs::metadata(parent)
                    .map(|metadata| !metadata.permissions().readonly())
                    .unwrap_or(false);
            doctor_check(
                &mut checks,
                "write permission",
                if writable { "pass" } else { "fail" },
                if writable {
                    "parent directory is writable"
                } else {
                    "parent directory is not writable"
                },
            );
            doctor_check(
                &mut checks,
                "format",
                "skip",
                "new catalog will use the supported format",
            );
            doctor_check(
                &mut checks,
                "migration",
                "skip",
                "new catalog needs no migration",
            );
            doctor_check(
                &mut checks,
                "snapshot",
                "pass",
                "empty catalog is ready to initialize",
            );
            doctor_check(
                &mut checks,
                "storage latency",
                "skip",
                "no storage read was needed",
            );
        }
    } else {
        let started = std::time::Instant::now();
        match resolve_catalog_with_opts_mode(&args.catalog, &S3Options::default(), false) {
            Ok((catalog_path, store)) => {
                doctor_check(&mut checks, "uri", "pass", "valid object-store URI");
                let mut objects = store.list(Some(&catalog_path));
                match objects.try_next().await {
                    Ok(Some(meta)) => {
                        doctor_check(
                            &mut checks,
                            "credentials",
                            "pass",
                            "object-store credentials accepted",
                        );
                        doctor_check(
                            &mut checks,
                            "connectivity",
                            "pass",
                            "object store is reachable",
                        );
                        doctor_check(
                            &mut checks,
                            "catalog prefix",
                            "pass",
                            "catalog objects exist",
                        );
                        match store.get(&meta.location).await {
                            Ok(_) => doctor_check(
                                &mut checks,
                                "read permission",
                                "pass",
                                "catalog object is readable",
                            ),
                            Err(error) => doctor_check(
                                &mut checks,
                                "read permission",
                                "fail",
                                error.to_string(),
                            ),
                        }
                        doctor_check(
                            &mut checks,
                            "write permission",
                            "skip",
                            "not probed because doctor never mutates a catalog",
                        );
                        location = Some(Ok((catalog_path, store)));
                    }
                    Ok(None) => {
                        doctor_check(
                            &mut checks,
                            "credentials",
                            "pass",
                            "object-store credentials accepted",
                        );
                        doctor_check(
                            &mut checks,
                            "connectivity",
                            "pass",
                            "object store is reachable",
                        );
                        doctor_check(
                            &mut checks,
                            "catalog prefix",
                            "fail",
                            "catalog prefix is empty or missing",
                        );
                        doctor_check(
                            &mut checks,
                            "read permission",
                            "skip",
                            "no catalog object exists to read",
                        );
                        doctor_check(
                            &mut checks,
                            "write permission",
                            "skip",
                            "not probed because doctor never mutates a catalog",
                        );
                    }
                    Err(error) => {
                        let message = error.to_string();
                        doctor_check(&mut checks, "credentials", "fail", &message);
                        doctor_check(&mut checks, "connectivity", "fail", message);
                        doctor_check(
                            &mut checks,
                            "catalog prefix",
                            "skip",
                            "object-store listing failed",
                        );
                        doctor_check(
                            &mut checks,
                            "read permission",
                            "skip",
                            "object-store listing failed",
                        );
                        doctor_check(
                            &mut checks,
                            "write permission",
                            "skip",
                            "not probed because doctor never mutates a catalog",
                        );
                    }
                }
                doctor_check(
                    &mut checks,
                    "storage latency",
                    "pass",
                    format!(
                        "catalog listing completed in {} ms",
                        started.elapsed().as_millis()
                    ),
                );
            }
            Err(error) => {
                doctor_check(&mut checks, "uri", "fail", error);
                doctor_check(&mut checks, "credentials", "skip", "URI validation failed");
                doctor_check(&mut checks, "connectivity", "skip", "URI validation failed");
                doctor_check(
                    &mut checks,
                    "catalog prefix",
                    "skip",
                    "URI validation failed",
                );
            }
        }
    }

    let tls_cert = setting(
        args.tls_cert,
        "ROCKLAKE_TLS_CERT",
        file_config.tls_cert,
        None,
        "TLS certificate",
    )?;
    let tls_key = setting(
        args.tls_key,
        "ROCKLAKE_TLS_KEY",
        file_config.tls_key,
        None,
        "TLS key",
    )?;
    let auth_user = setting(
        args.auth_user,
        "ROCKLAKE_AUTH_USER",
        file_config.auth_user,
        None,
        "auth user",
    )?;
    let tls = tls_cert.is_some() && tls_key.is_some();
    if !bind.ip().is_loopback() && !tls && auth_user.is_none() {
        let warning = "listener is non-loopback without TLS or authentication".to_string();
        warnings.push(warning.clone());
        doctor_check(&mut checks, "runtime safety", "fail", warning);
    } else {
        doctor_check(
            &mut checks,
            "runtime safety",
            "pass",
            "listener configuration is acceptable",
        );
    }
    doctor_check(
        &mut checks,
        "reader/writer eligibility",
        "pass",
        if mode == "reader" {
            "reader mode uses no writer epoch"
        } else {
            "writer mode can acquire the epoch during serve"
        },
    );
    if !checks
        .iter()
        .any(|check| check.name == "DuckLake compatibility")
    {
        doctor_check(
            &mut checks,
            "DuckLake compatibility",
            "pass",
            "DuckLake 1.0 catalog layout is supported",
        );
    }
    if !checks.iter().any(|check| check.name == "storage latency") {
        doctor_check(
            &mut checks,
            "storage latency",
            "skip",
            "local filesystem latency was not measured",
        );
    }

    if let Some(Ok((catalog_path, store))) = location {
        match slatedb::Db::open(catalog_path, store).await {
            Ok(db) => match rocklake_catalog::inspect::inspect_snapshot(&db).await {
                Ok(info) => {
                    doctor_check(
                        &mut checks,
                        "format",
                        if info.format_version == rocklake_core::tags::CATALOG_FORMAT_VERSION {
                            "pass"
                        } else {
                            "fail"
                        },
                        format!(
                            "catalog format {} (expected {})",
                            info.format_version,
                            rocklake_core::tags::CATALOG_FORMAT_VERSION
                        ),
                    );
                    match rocklake_catalog::init::verify_migrations_complete(&db).await {
                        Ok(()) => doctor_check(
                            &mut checks,
                            "migration",
                            "pass",
                            "key migrations complete",
                        ),
                        Err(error) => {
                            doctor_check(&mut checks, "migration", "fail", error.to_string())
                        }
                    }
                    doctor_check(
                        &mut checks,
                        "snapshot",
                        "pass",
                        format!("latest committed snapshot {}", info.latest_snapshot_id),
                    );
                }
                Err(error) => doctor_check(&mut checks, "catalog state", "fail", error.to_string()),
            },
            Err(error) => doctor_check(&mut checks, "catalog open", "fail", error.to_string()),
        }
    }

    let ready = checks.iter().all(|check| check.status != "fail");
    let report = DoctorReport {
        schema_version: 1,
        rocklake_version: env!("CARGO_PKG_VERSION"),
        catalog: args.catalog,
        mode,
        ready,
        checks,
        warnings,
    };
    match args.output {
        cli::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        cli::OutputFormat::Human => {
            println!("RockLake Doctor {}", report.rocklake_version);
            println!("Catalog: {}", report.catalog);
            println!("Mode: {}", report.mode);
            for check in &report.checks {
                println!(
                    "[{:<4}] {:<24} {}",
                    check.status.to_uppercase(),
                    check.name,
                    check.message
                );
            }
            println!(
                "Status: {}",
                if report.ready { "READY" } else { "NOT READY" }
            );
        }
    }
    if !report.ready {
        std::process::exit(1);
    }
    Ok(())
}

async fn cmd_config(
    command: cli::ConfigSubcommand,
    config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        cli::ConfigSubcommand::Example => print!("{}", config::example()),
        cli::ConfigSubcommand::Check(args) => {
            let path = args
                .file
                .as_deref()
                .or(config_path)
                .ok_or("no config file selected; use --file or --config")?;
            let (path, file_config) = config::load(Some(path))?;
            warn_deprecated_limits(
                file_config.stream_queue_depth.is_some() || file_config.max_buffered_rows.is_some(),
            );
            validate_config(&file_config)?;
            let path = path.expect("explicit config path");
            match args.output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "valid": true,
                        "path": path,
                        "effective": redacted_config(&file_config)
                    })
                ),
                cli::OutputFormat::Human => {
                    println!("Valid configuration: {}", path.display())
                }
            }
        }
    }
    Ok(())
}

fn validate_config(config: &config::ConfigFile) -> Result<(), String> {
    if config.auth_password.is_some() && config.auth_password_file.is_some() {
        return Err("auth_password and auth_password_file are mutually exclusive".to_string());
    }
    if config.encryption_key.is_some() && config.encryption_key_file.is_some() {
        return Err("encryption_key and encryption_key_file are mutually exclusive".to_string());
    }
    if let Some(mode) = &config.mode {
        if mode != "writer" && mode != "reader" {
            return Err("mode must be writer or reader".to_string());
        }
    }
    if let Some(cost_mode) = &config.cost_mode {
        if !["conservative", "balanced", "latency"].contains(&cost_mode.as_str()) {
            return Err("cost_mode must be conservative, balanced, or latency".to_string());
        }
    }
    if let Some(bind) = &config.bind {
        bind.parse::<SocketAddr>()
            .map_err(|e| format!("invalid bind: {e}"))?;
    }
    if config.max_sessions == Some(0)
        || config.datafusion_bridge_queue_depth == Some(0)
        || config.max_active_scans == Some(0)
        || config.stream_queue_depth == Some(0)
        || config.max_buffered_rows == Some(0)
        || config.max_response_bytes == Some(0)
        || config.slow_operation_threshold_ms == Some(0)
    {
        return Err("numeric limits must be greater than zero".to_string());
    }
    if config.tls_required == Some(true) && (config.tls_cert.is_none() || config.tls_key.is_none())
    {
        return Err("tls_required needs tls_cert and tls_key".to_string());
    }
    if let Some(key) = config.encryption_key.as_deref() {
        rocklake_catalog::EncryptionConfig::from_hex(key).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn redacted_config(config: &config::ConfigFile) -> serde_json::Value {
    serde_json::json!({
        "catalog": config.catalog,
        "bind": config.bind,
        "max_sessions": config.max_sessions,
        "metrics_port": config.metrics_port,
        "metrics_path": config.metrics_path,
        "tls_cert": config.tls_cert,
        "tls_key": config.tls_key,
        "tls_required": config.tls_required,
        "auth_user": config.auth_user,
        "auth_password": config.auth_password.as_ref().map(|_| "[redacted]"),
        "auth_password_file": config.auth_password_file,
        "mode": config.mode,
        "cost_mode": config.cost_mode,
        "s3_endpoint": config.s3_endpoint,
        "s3_path_style": config.s3_path_style,
        "encryption_key": config.encryption_key.as_ref().map(|_| "[redacted]"),
        "encryption_key_file": config.encryption_key_file,
        "extension_schemas": config.extension_schemas,
        "otlp_endpoint": config.otlp_endpoint,
        "idle_connection_timeout": config.idle_connection_timeout,
        "drain_timeout": config.drain_timeout,
        "datafusion_bridge_queue_depth": config.datafusion_bridge_queue_depth,
        "max_active_scans": config.max_active_scans,
        "stream_queue_depth": config.stream_queue_depth,
        "max_buffered_rows": config.max_buffered_rows,
        "max_response_bytes": config.max_response_bytes,
        "slow_operation_threshold_ms": config.slow_operation_threshold_ms,
    })
}

async fn cmd_backup(command: cli::BackupSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        cli::BackupSubcommand::Create(args) => {
            let (catalog_path, object_store) =
                resolve_catalog_with_opts_mode(&args.catalog, &S3Options::default(), false)?;
            let db = slatedb::Db::open(catalog_path, object_store).await?;
            let info =
                rocklake_catalog::create_backup(&db, &args.out, &args.catalog, args.snapshot_id)
                    .await?;
            db.close().await?;
            println!("Backup created: {}", info.path.display());
            println!("  Snapshot: {}", info.manifest.snapshot_id);
            println!("  Rows: {}", info.manifest.row_count);
            println!("  SHA-256: {}", info.manifest.sha256);
        }
        cli::BackupSubcommand::Inspect(args) => {
            let info = rocklake_catalog::inspect_backup(&args.backup).await?;
            match args.output {
                cli::OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&info.manifest)?)
                }
                cli::OutputFormat::Human => {
                    println!("Backup: {}", info.path.display());
                    println!("  Version: {}", info.manifest.version);
                    println!("  Source: {}", info.manifest.source_identity);
                    println!("  Snapshot: {}", info.manifest.snapshot_id);
                    println!("  Rows: {}", info.manifest.row_count);
                    println!("  Bytes: {}", info.manifest.byte_count);
                    println!("  SHA-256: {}", info.manifest.sha256);
                }
            }
        }
    }
    Ok(())
}

async fn cmd_restore(command: cli::RestoreSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let (args, apply) = match command {
        cli::RestoreSubcommand::Plan(args) => (args, false),
        cli::RestoreSubcommand::Apply(args) => (args, true),
    };
    let backup = rocklake_catalog::inspect_backup(&args.backup).await?;
    let local_target_missing = !apply
        && args
            .catalog
            .strip_prefix("file://")
            .or_else(|| (!args.catalog.contains("://")).then_some(args.catalog.as_str()))
            .is_some_and(|path| !std::path::Path::new(path).exists());
    if local_target_missing {
        let plan = serde_json::json!({
            "schema_version": 1,
            "backup": args.backup,
            "catalog": args.catalog,
            "snapshot_id": backup.manifest.snapshot_id,
            "rows": backup.manifest.row_count,
            "target_empty": true,
            "action": "import",
        });
        match args.output {
            cli::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
            cli::OutputFormat::Human => {
                println!("Restore plan:\n{}", serde_json::to_string_pretty(&plan)?)
            }
        }
        return Ok(());
    }
    let (catalog_path, object_store) =
        resolve_catalog_with_opts_mode(&args.catalog, &S3Options::default(), apply)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;
    let mut existing = db.scan::<&[u8], _>(std::ops::RangeFull).await?;
    let target_empty = existing
        .next()
        .await
        .map_err(|e| format!("scan restore target: {e}"))?
        .is_none();
    if !target_empty && apply && !args.overwrite {
        db.close().await?;
        return Err(
            "restore target is not empty; pass --overwrite explicitly or use a new catalog path"
                .into(),
        );
    }
    let plan = serde_json::json!({
        "schema_version": 1,
        "backup": args.backup,
        "catalog": args.catalog,
        "snapshot_id": backup.manifest.snapshot_id,
        "rows": backup.manifest.row_count,
        "target_empty": target_empty,
        "action": if apply && !target_empty && args.overwrite {
            "overwrite and import"
        } else if apply {
            "import"
        } else if target_empty {
            "no mutation"
        } else {
            "refused: target is not empty"
        },
    });
    if !apply {
        db.close().await?;
        match args.output {
            cli::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
            cli::OutputFormat::Human => {
                println!("Restore plan:\n{}", serde_json::to_string_pretty(&plan)?)
            }
        }
        return Ok(());
    }
    let data_path = args.backup.join("catalog.ndjson");
    let file = std::fs::File::open(&data_path)?;
    if !target_empty {
        let mut delete_batch = slatedb::WriteBatch::new();
        let mut keys_deleted = 0usize;
        let mut keys = db.scan::<&[u8], _>(std::ops::RangeFull).await?;
        while let Some(kv) = keys
            .next()
            .await
            .map_err(|e| format!("scan restore target for overwrite: {e}"))?
        {
            delete_batch.delete(&kv.key);
            keys_deleted += 1;
        }
        if keys_deleted > 0 {
            db.write(delete_batch).await?;
        }
    }
    let result =
        rocklake_catalog::export::import_catalog(&db, std::io::BufReader::new(file)).await?;
    let restored = rocklake_catalog::inspect::inspect_snapshot(&db).await?;
    db.close().await?;
    if restored.latest_snapshot_id != backup.manifest.snapshot_id {
        return Err(format!(
            "restore verification failed: restored snapshot {} differs from backup snapshot {}",
            restored.latest_snapshot_id, backup.manifest.snapshot_id
        )
        .into());
    }
    match args.output {
        cli::OutputFormat::Json => println!(
            "{}",
            serde_json::json!({
                "schema_version": 1,
                "restored": true,
                "rows_imported": result.rows_imported,
                "tables_imported": result.tables_imported,
                "verified": true
            })
        ),
        cli::OutputFormat::Human => println!(
            "Restore applied: {} rows imported and verified",
            result.rows_imported
        ),
    }
    Ok(())
}

// ─── sweep-orphans (v0.39.0) ───────────────────────────────────────────────

/// Identify (and optionally delete) orphan Parquet files in object storage.
///
/// Example:
///   rocklake sweep-orphans --catalog ./my-catalog --data-root ./data/
///   rocklake sweep-orphans --catalog ./my-catalog --data-root s3://bucket/data/ --grace-period-hours 48
///   rocklake sweep-orphans --catalog ./my-catalog --data-root ./data/ --apply
async fn cmd_sweep_orphans(args: cli::SweepOrphansArgs) -> Result<(), Box<dyn std::error::Error>> {
    let catalog_url = args.catalog;
    let data_root = args.data_root;
    let grace_period_hours = args.grace_period_hours;
    let apply = args.apply;
    let output = args.output;

    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store.clone()).await?;

    let config = rocklake_catalog::SweepOrphansConfig {
        grace_period_hours,
        apply,
        data_root: data_root.clone(),
    };

    let result = rocklake_catalog::sweep_orphans(&db, object_store, &config).await?;
    db.close().await?;

    match output {
        cli::OutputFormat::Json => println!(
            "{}",
            serde_json::json!({
                "schema_version": 1,
                "data_root": data_root,
                "files_scanned": result.total_scanned,
                "orphan_files": &result.orphan_files,
                "files_deleted": result.deleted,
                "deletion_failures": &result.deletion_failures,
                "grace_period_hours": grace_period_hours,
                "applied": apply
            })
        ),
        cli::OutputFormat::Human => {
            if apply {
                println!("Sweep complete (--apply mode):");
            } else {
                println!("Sweep complete (dry-run — use --apply to delete):");
            }
            println!("  Data root:          {data_root}");
            println!("  Files scanned:      {}", result.total_scanned);
            println!("  Orphan files found: {}", result.orphan_files.len());
            println!("  Files deleted:      {}", result.deleted);
            println!("  Deletion failures:  {}", result.deletion_failures.len());
            println!("  Grace period:       {grace_period_hours}h");
            if !result.orphan_files.is_empty() {
                println!("\nOrphan files:");
                for f in &result.orphan_files {
                    println!("  {f}");
                }
            }
        }
    }

    if !result.deletion_failures.is_empty() {
        return Err(format!(
            "sweep incomplete: {} object deletions failed",
            result.deletion_failures.len()
        )
        .into());
    }

    Ok(())
}
