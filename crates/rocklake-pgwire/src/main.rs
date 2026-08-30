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

    match cli.command {
        Commands::Completions(args) => {
            let mut cmd = cli::Cli::command();
            generate(args.shell, &mut cmd, "rocklake", &mut io::stdout());
        }
        Commands::Serve(args) => cmd_serve(*args).await?,
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

async fn cmd_serve(args: cli::ServeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mode = if args.read_only {
        "reader".to_string()
    } else {
        args.mode
    };
    let auth_password = read_secret(
        args.auth_password,
        args.auth_password_file.as_deref(),
        "ROCKLAKE_AUTH_PASSWORD_FILE",
    )?;
    let encryption_key = read_secret(
        args.encryption_key,
        args.encryption_key_file.as_deref(),
        "ROCKLAKE_ENCRYPTION_KEY_FILE",
    )?;
    let config = ServeConfig {
        catalog_url: args.catalog,
        bind_addr: args
            .bind
            .parse()
            .map_err(|e| format!("invalid bind address: {e}"))?,
        max_sessions: args.max_sessions,
        metrics_port: args.metrics_port,
        metrics_path: args.metrics_path,
        tls_cert: args.tls_cert,
        tls_key: args.tls_key,
        tls_required: args.tls_required,
        auth_username: args.auth_user,
        auth_password,
        mode,
        cost_mode: args
            .cost_mode
            .parse()
            .map_err(|e| format!("invalid cost mode: {e}"))?,
        s3_endpoint: args.s3_endpoint,
        s3_path_style: args.s3_path_style,
        encryption_key,
        extension_schemas: if args.extension_schemas.is_empty() {
            vec!["public".to_string(), "pgtrickle".to_string()]
        } else {
            args.extension_schemas
        },
        otlp_endpoint: args.otlp_endpoint,
        idle_connection_timeout_secs: args.idle_connection_timeout,
        drain_timeout_secs: args.drain_timeout,
        datafusion_bridge_queue_depth: args.datafusion_bridge_queue_depth,
    };

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
        encryption: config
            .encryption_key
            .as_deref()
            .map(rocklake_catalog::EncryptionConfig::from_hex)
            .transpose()
            .map_err(|e| format!("--encryption-key: {e}"))?,
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
        max_active_scans: 25,
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
    use super::read_secret;

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
}

// ─── gc ────────────────────────────────────────────────────────────────────

async fn cmd_gc(command: cli::GcSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_url, retention_days, apply) = match command {
        cli::GcSubcommand::Plan(args) => (args.catalog, args.retention_days, false),
        cli::GcSubcommand::Apply(args) => (args.catalog, args.retention_days, true),
    };
    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    if !apply {
        let plan = rocklake_catalog::gc::gc_plan(&db, retention_days).await?;
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
    } else {
        let plan = rocklake_catalog::gc::gc_plan(&db, retention_days).await?;
        let result = rocklake_catalog::gc::gc_apply(&db, plan.proposed_retain_from).await?;
        println!("GC Applied:");
        println!("  Previous retain-from: {}", result.previous_retain_from);
        println!("  New retain-from: {}", result.new_retain_from);
        println!("  Snapshots hidden: {}", result.snapshots_hidden);
    }

    db.close().await?;
    Ok(())
}

// ─── excise ────────────────────────────────────────────────────────────────

async fn cmd_excise(command: cli::ExciseSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_url, before, apply) = match command {
        cli::ExciseSubcommand::Plan(args) => (args.catalog, args.before, false),
        cli::ExciseSubcommand::Apply(args) => (args.catalog, args.before, true),
    };
    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    if !apply {
        let plan = rocklake_catalog::excise::excise_plan(&db, before).await?;
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
    } else {
        let result = rocklake_catalog::excise::excise_apply(&db, before, "operator").await?;
        println!("Excise Applied:");
        println!("  Keys deleted: {}", result.keys_deleted);
        println!("  Keys failed: {}", result.keys_failed);
        println!("  Audit entry ID: {}", result.audit_entry_id);
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
            let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
            let db = slatedb::Db::open(catalog_path, object_store).await?;

            let result = rocklake_catalog::inspect::inspect_snapshot(&db).await?;
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

            db.close().await?;
        }
        cli::InspectSubcommand::ApiCosts(args) => {
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

            report.print();
        }
        cli::InspectSubcommand::CacheUtilization(args) => {
            let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
            let db = slatedb::Db::open(catalog_path, object_store).await?;
            let state = rocklake_catalog::inspect::inspect_snapshot(&db).await?;
            db.close().await?;

            let stats =
                rocklake_catalog::cache_utilization(256, state.data_file_count, state.column_count)
                    .await;
            stats.print();
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
    let (catalog_path, object_store) = resolve_catalog(catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store.clone()).await?;

    match command {
        cli::VerifySubcommand::Catalog(_) => {
            let result = rocklake_catalog::verify::verify_catalog(&db).await?;
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
        cli::VerifySubcommand::DataFiles(_) => {
            let result = rocklake_catalog::cleanup::verify_data_files(&db, &object_store).await?;
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

    db.close().await?;
    Ok(())
}

// ─── repair ────────────────────────────────────────────────────────────────

async fn cmd_repair(args: cli::RepairArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let plan = rocklake_catalog::repair::repair_plan(&db).await?;

    if plan.is_empty() {
        println!("No repairs needed. Catalog is healthy.");
    } else {
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

        if args.apply && !plan.has_unrecoverable() {
            let result = rocklake_catalog::repair::repair_apply(&db, &plan).await?;
            println!("Repair Applied:");
            println!("  Actions applied: {}", result.actions_applied);
            println!("  Actions failed: {}", result.actions_failed);
        } else if !args.apply {
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
    let json_output = args.json;
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

    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store.clone()).await?;

    let config = rocklake_catalog::SweepOrphansConfig {
        grace_period_hours,
        apply,
        data_root: data_root.clone(),
    };

    let result = rocklake_catalog::sweep_orphans(&db, object_store, &config).await?;
    db.close().await?;

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

    if !result.deletion_failures.is_empty() {
        return Err(format!(
            "sweep incomplete: {} object deletions failed",
            result.deletion_failures.len()
        )
        .into());
    }

    Ok(())
}
