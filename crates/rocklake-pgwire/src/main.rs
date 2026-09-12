//! `rocklake` — CLI binary with all operational commands.
//!
//! Commands:
//!   serve, gc, excise, checkpoint, export, import, pg-migrate,
//!   rebuild, inspect, verify, repair,
//!   warmup, migrate, corpus, tune,
//!   migrate-from-ducklake, export-catalog,
//!   diagnose, sweep-orphans, capacity
//!
//! Run `rocklake --help` or `rocklake <command> --help` for full usage.

mod cli;
mod config;

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use clap::{CommandFactory as _, Parser as _};
use clap_complete::generate;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use rocklake_catalog::metrics::CatalogMetrics;
use rocklake_catalog::{CatalogStore, OpenOptions};
use rocklake_pgwire::server::{
    run_server_with_mode, run_server_with_router_and_catalog_and_auth, MultiPrincipalAuth,
    ServerConfig,
};
use rocklake_router::CatalogLocation;

const MAIN_THREAD_STACK_SIZE: usize = 8 * 1024 * 1024; // 8 MiB

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        let json = args.iter().enumerate().any(|(index, arg)| {
            arg == "--output=json"
                || (arg == "--output" && args.get(index + 1).is_some_and(|value| value == "json"))
        });
        return print_version(json);
    }

    let builder = std::thread::Builder::new()
        .name("rocklake-main".into())
        .stack_size(MAIN_THREAD_STACK_SIZE);

    let handle = builder.spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_stack_size(MAIN_THREAD_STACK_SIZE)
            .build()
            .map_err(|e| e.to_string())?;
        rt.block_on(async_main()).map_err(|e| e.to_string())
    })?;

    match handle.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(err.into()),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn print_version(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if json {
        println!("{}", version_json());
    } else {
        println!("RockLake {}", env!("CARGO_PKG_VERSION"));
    }
    Ok(())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
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
        Commands::Status(args) => cmd_status(args, config_path.as_deref()).await?,
        Commands::Support(command) => cmd_support(command, config_path.as_deref()).await?,
        Commands::Capacity(command) => cmd_capacity(command).await?,
        Commands::Catalog(command) => match command {
            cli::CatalogSubcommand::Backup(sub) => cmd_backup(sub).await?,
            cli::CatalogSubcommand::Restore(sub) => cmd_restore(sub).await?,
            cli::CatalogSubcommand::Gc(sub) => cmd_gc(sub).await?,
            cli::CatalogSubcommand::Excise(sub) => cmd_excise(sub).await?,
            cli::CatalogSubcommand::Checkpoint(sub) => cmd_checkpoint(sub).await?,
            cli::CatalogSubcommand::Export(args) => cmd_export(args).await?,
            cli::CatalogSubcommand::Import(args) => cmd_import(args).await?,
            cli::CatalogSubcommand::ExportCatalog(args) => cmd_export_catalog(args).await?,
            cli::CatalogSubcommand::Migrate(args) => cmd_migrate(args).await?,
            cli::CatalogSubcommand::Verify(sub) => cmd_verify(sub).await?,
            cli::CatalogSubcommand::Repair(args) => cmd_repair(args).await?,
            cli::CatalogSubcommand::Jobs(sub) => cmd_jobs(sub).await?,
            cli::CatalogSubcommand::Maintenance(sub) => cmd_maintenance(sub).await?,
            cli::CatalogSubcommand::Recovery(sub) => cmd_recovery(sub).await?,
            cli::CatalogSubcommand::BackupSet(sub) => {
                cmd_backup_set(sub, config_path.as_deref()).await?
            }
        },
        Commands::Catalogs(command) => cmd_catalogs(command, config_path.as_deref()).await?,
        Commands::Registry(command) => cmd_registry(command, config_path.as_deref()).await?,
        Commands::Debug(command) => match command {
            cli::DebugSubcommand::Diagnose(args) => cmd_diagnose(args).await?,
            cli::DebugSubcommand::Inspect(sub) => cmd_inspect(sub).await?,
            cli::DebugSubcommand::Corpus(sub) => cmd_corpus(sub).await?,
            cli::DebugSubcommand::Rebuild(args) => cmd_rebuild(args).await?,
            cli::DebugSubcommand::SweepOrphans(args) => cmd_sweep_orphans(args).await?,
            cli::DebugSubcommand::PgMigrate(args) => cmd_pg_migrate(args).await?,
            cli::DebugSubcommand::Tune(args) => cmd_tune(args).await?,
            cli::DebugSubcommand::Warmup(args) => cmd_warmup(args).await?,
            cli::DebugSubcommand::MigrateFromDucklake(args) => {
                cmd_migrate_from_ducklake(args).await?
            }
        },
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

// ─── jobs ─────────────────────────────────────────────────────────────────

async fn cmd_jobs(command: cli::JobSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let catalog_url = match &command {
        cli::JobSubcommand::List(args) => &args.catalog,
        cli::JobSubcommand::Status(args) => &args.catalog,
        cli::JobSubcommand::Cancel(args) => &args.catalog,
        cli::JobSubcommand::Resume(args) => &args.catalog,
    };
    let output = match &command {
        cli::JobSubcommand::List(args) => Some(args.output),
        cli::JobSubcommand::Status(args) => Some(args.output),
        cli::JobSubcommand::Cancel(_) | cli::JobSubcommand::Resume(_) => None,
    };
    let (catalog_path, object_store) = resolve_catalog(catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;
    let ledger = rocklake_catalog::JobLedger::new(&db);

    match command {
        cli::JobSubcommand::List(_) => {
            let jobs = ledger.list().await?;
            match output.expect("list output") {
                cli::OutputFormat::Json => {
                    println!("{}", serde_json::json!({"schema_version": 1, "jobs": jobs}))
                }
                cli::OutputFormat::Human => {
                    if jobs.is_empty() {
                        println!("No administrative jobs.");
                    } else {
                        for job in jobs {
                            println!(
                                "{}  {:?}  {:?}  {}/{}",
                                job.id,
                                job.kind,
                                job.state,
                                job.progress.items_done,
                                job.progress
                                    .items_total
                                    .map_or_else(|| "?".to_string(), |total| total.to_string())
                            );
                        }
                    }
                }
            }
        }
        cli::JobSubcommand::Status(args) => {
            let id: rocklake_catalog::JobId = args.id.parse()?;
            let job = ledger
                .get(&id)
                .await?
                .ok_or_else(|| format!("job {id} not found"))?;
            match args.output {
                cli::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&job)?),
                cli::OutputFormat::Human => print_job_human(&job),
            }
        }
        cli::JobSubcommand::Cancel(args) => {
            let id: rocklake_catalog::JobId = args.id.parse()?;
            let job = ledger.request_cancel(&id).await?;
            println!(
                "Cancellation requested for job {} ({:?}).",
                job.id, job.state
            );
        }
        cli::JobSubcommand::Resume(args) => {
            let id: rocklake_catalog::JobId = args.id.parse()?;
            let job = ledger.resume(&id).await?;
            println!("Job {} requeued ({:?}).", job.id, job.state);
        }
    }

    db.close().await?;
    Ok(())
}

async fn cmd_maintenance(
    command: cli::MaintenanceSubcommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let catalog_url = match &command {
        cli::MaintenanceSubcommand::Schedule(args) => &args.catalog,
        cli::MaintenanceSubcommand::List(args) => &args.catalog,
        cli::MaintenanceSubcommand::Remove(args) => &args.catalog,
        cli::MaintenanceSubcommand::Run(args) => &args.catalog,
    };
    let (catalog_path, object_store) = resolve_catalog(catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;
    let scheduler = rocklake_catalog::MaintenanceScheduler::new(&db);

    match command {
        cli::MaintenanceSubcommand::Schedule(args) => {
            let window = match (args.window_start_minute, args.window_end_minute) {
                (None, None) => None,
                (Some(start_minute), Some(end_minute)) => {
                    Some(rocklake_catalog::MaintenancePeriod {
                        start_minute,
                        end_minute,
                    })
                }
                _ => return Err("maintenance windows require both start and end minutes".into()),
            };
            let task = match args.task {
                cli::MaintenanceTaskArg::Backup => rocklake_catalog::MaintenanceTask::Backup,
                cli::MaintenanceTaskArg::Verification => {
                    rocklake_catalog::MaintenanceTask::Verification
                }
                cli::MaintenanceTaskArg::Retention => rocklake_catalog::MaintenanceTask::Retention,
                cli::MaintenanceTaskArg::Checkpoint => {
                    rocklake_catalog::MaintenanceTask::Checkpoint
                }
                cli::MaintenanceTaskArg::OrphanSweep => {
                    rocklake_catalog::MaintenanceTask::OrphanSweep
                }
            };
            scheduler
                .upsert(rocklake_catalog::MaintenanceSchedule {
                    id: args.id,
                    task,
                    interval_seconds: args.interval_seconds,
                    next_run_at_unix_ms: args.next_run_at_ms,
                    enabled: true,
                    window,
                    blackouts: Vec::new(),
                })
                .await?;
            println!("Maintenance schedule saved.");
        }
        cli::MaintenanceSubcommand::List(args) => {
            let schedules = scheduler.list().await?;
            match args.output {
                cli::OutputFormat::Json => {
                    println!("{}", serde_json::json!({"schedules": schedules}))
                }
                cli::OutputFormat::Human => {
                    for schedule in schedules {
                        println!(
                            "{}  {:?}  every {}s  next {}",
                            schedule.id,
                            schedule.task,
                            schedule.interval_seconds,
                            schedule.next_run_at_unix_ms
                        );
                    }
                }
            }
        }
        cli::MaintenanceSubcommand::Remove(args) => {
            scheduler.remove(&args.id).await?;
            println!("Maintenance schedule removed: {}", args.id);
        }
        cli::MaintenanceSubcommand::Run(args) => {
            let now = args.now_ms.unwrap_or(unix_now_ms()?);
            let due = scheduler.claim_due(now, args.limit).await?;
            let ledger = rocklake_catalog::JobLedger::new(&db);
            let mut jobs = Vec::with_capacity(due.len());
            for item in due {
                let record = ledger
                    .create(rocklake_catalog::JobRequest::new(
                        item.job_kind,
                        None,
                        serde_json::json!({"schedule_id": item.schedule.id, "claimed_at_ms": now}),
                        None,
                    ))
                    .await?;
                jobs.push(record.id.to_string());
            }
            match args.output {
                cli::OutputFormat::Json => println!("{}", serde_json::json!({"jobs": jobs})),
                cli::OutputFormat::Human => {
                    for job in jobs {
                        println!("Maintenance job queued: {job}");
                    }
                }
            }
        }
    }
    db.close().await?;
    Ok(())
}

async fn cmd_backup_set(
    command: cli::BackupSetSubcommand,
    config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_, file_config) = config::load(config_path)?;
    match command {
        cli::BackupSetSubcommand::Create(args) => {
            let registry = open_registry(&args.registry, &file_config).await?;
            let catalog_ids = args
                .catalog_ids
                .iter()
                .map(|id| id.parse())
                .collect::<Result<Vec<rocklake_router::CatalogId>, _>>()?;
            let info = rocklake_router::create_backup_set(
                &registry,
                &args.output,
                rocklake_router::BackupSetOptions {
                    catalog_ids,
                    include_data_inventory: args.include_data,
                    verify_data: args.verify_data,
                    router: router_open_options(&file_config),
                },
            )
            .await?;
            registry.close().await?;
            println!(
                "Backup set created: {} (generation {}, {} catalogs)",
                info.path.display(),
                info.manifest.registry_generation,
                info.manifest.catalogs.len()
            );
        }
        cli::BackupSetSubcommand::Inspect(args) => {
            let info = rocklake_router::inspect_backup_set(&args.input).await?;
            match args.output {
                cli::OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&info.manifest)?)
                }
                cli::OutputFormat::Human => println!(
                    "Backup set: {}\n  Generation: {}\n  Catalogs: {}",
                    info.path.display(),
                    info.manifest.registry_generation,
                    info.manifest.catalogs.len()
                ),
            }
        }
        cli::BackupSetSubcommand::Plan(args) => {
            let info = rocklake_router::inspect_backup_set(&args.input).await?;
            let token = backup_overwrite_token(&info.manifest, &args.registry);
            let existing = registry_exists(&args.registry, &file_config).await?;
            let plan = backup_set_restore_plan(&info, &args, existing, &token);
            print_backup_set_plan(plan, args.output)?;
        }
        cli::BackupSetSubcommand::Apply(args) => {
            let info = rocklake_router::inspect_backup_set(&args.input).await?;
            let token = backup_overwrite_token(&info.manifest, &args.registry);
            let existing = registry_exists(&args.registry, &file_config).await?;
            if existing {
                if args.overwrite_token.as_deref() != Some(token.as_str()) {
                    return Err(format!(
                        "restore destination exists; run plan and pass --overwrite-token {token}"
                    )
                    .into());
                }
                return Err(
                    "existing backup-set destinations are not overwritten; restore to a new registry location"
                        .into(),
                );
            }
            let registry = open_registry(&args.registry, &file_config).await?;
            registry.init().await?;
            let s3_options = S3Options {
                endpoint: file_config.s3_endpoint.clone(),
                path_style: file_config.s3_path_style.unwrap_or(false),
            };
            let mut restored = Vec::new();
            for child in &info.manifest.catalogs {
                let catalog_location = join_location(&args.catalog_root, &child.id.to_string());
                let data_location = join_location(&args.data_root, &child.id.to_string());
                let (path, store) =
                    resolve_catalog_with_opts_mode(&catalog_location, &s3_options, true)?;
                let db = slatedb::Db::open(path, store).await?;
                let input =
                    std::fs::File::open(args.input.join(&child.backup).join("catalog.ndjson"))?;
                let catalog_location_for_job = catalog_location.clone();
                let result = run_job(
                    &db,
                    rocklake_catalog::JobKind::Restore,
                    serde_json::json!({
                        "backup_set": args.input,
                        "catalog_id": child.id,
                        "catalog": catalog_location_for_job
                    }),
                    None,
                    |job_db| async move {
                        let imported = rocklake_catalog::export::import_catalog(
                            &job_db,
                            std::io::BufReader::new(input),
                        )
                        .await?;
                        let verified = rocklake_catalog::verify::verify_catalog(&job_db).await?;
                        if !verified.is_ok() {
                            return Err(format!(
                                "restored catalog {} failed verification: {:?}",
                                child.id, verified.errors
                            )
                            .into());
                        }
                        Ok(imported)
                    },
                )
                .await?;
                db.close().await?;
                registry
                    .create(
                        rocklake_router::restored_catalog_config(
                            child,
                            catalog_location,
                            data_location,
                        )?,
                        &format!("restore-{}", child.id),
                    )
                    .await?;
                restored.push(serde_json::json!({
                    "catalog_id": child.id,
                    "rows_imported": result.0.rows_imported,
                    "job_id": result.1
                }));
            }
            match args.output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({"restored": restored, "published_as": "read_only"})
                ),
                cli::OutputFormat::Human => println!(
                    "Backup set restored and verified: {} catalogs published read-only.",
                    restored.len()
                ),
            }
        }
    }
    Ok(())
}

async fn cmd_recovery(command: cli::RecoverySubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        cli::RecoverySubcommand::Report(args) => {
            let drill = match args.drill {
                cli::RecoveryDrillArg::LostProcess => rocklake_catalog::RecoveryDrill::LostProcess,
                cli::RecoveryDrillArg::LostRegistryPrefix => {
                    rocklake_catalog::RecoveryDrill::LostRegistryPrefix
                }
                cli::RecoveryDrillArg::AccidentalRouteDeletion => {
                    rocklake_catalog::RecoveryDrill::AccidentalRouteDeletion
                }
                cli::RecoveryDrillArg::DamagedCatalogPrefix => {
                    rocklake_catalog::RecoveryDrill::DamagedCatalogPrefix
                }
                cli::RecoveryDrillArg::LostCredentials => {
                    rocklake_catalog::RecoveryDrill::LostCredentials
                }
                cli::RecoveryDrillArg::RegionRestore => {
                    rocklake_catalog::RecoveryDrill::RegionRestore
                }
            };
            let report = rocklake_catalog::RecoveryReport::completed(
                drill,
                args.started_at,
                args.rpo_seconds,
                args.rto_seconds,
                args.verified,
                args.details,
            );
            report.write_json(&args.output).await?;
            println!("Recovery report written: {}", args.output.display());
        }
    }
    Ok(())
}

fn backup_overwrite_token(manifest: &rocklake_router::BackupSetManifest, registry: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(manifest.registry_generation.to_string().as_bytes());
    digest.update(b":");
    digest.update(registry.as_bytes());
    let suffix = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("restore-{suffix}")
}

async fn registry_exists(
    location: &str,
    config: &config::ConfigFile,
) -> Result<bool, Box<dyn std::error::Error>> {
    let registry = open_registry(location, config).await?;
    let result = registry.snapshot().await;
    registry.close().await?;
    match result {
        Ok(_) => Ok(true),
        Err(rocklake_router::RegistryError::NotInitialized) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn join_location(root: &str, child: &str) -> String {
    format!("{}/{}", root.trim_end_matches('/'), child)
}

fn backup_set_restore_plan(
    info: &rocklake_router::BackupSetInfo,
    args: &cli::BackupSetRestoreArgs,
    existing: bool,
    token: &str,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "backup_set": info.path,
        "registry": args.registry,
        "registry_generation": info.manifest.registry_generation,
        "catalogs": info.manifest.catalogs.iter().map(|catalog| serde_json::json!({
            "id": catalog.id,
            "catalog": join_location(&args.catalog_root, &catalog.id.to_string()),
            "data": join_location(&args.data_root, &catalog.id.to_string()),
            "published_as": "read_only"
        })).collect::<Vec<_>>(),
        "destination_exists": existing,
        "overwrite_token": existing.then_some(token),
        "action": if existing { "requires_explicit_overwrite_token" } else { "create_new_prefixes" }
    })
}

fn print_backup_set_plan(
    plan: serde_json::Value,
    output: cli::OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match output {
        cli::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
        cli::OutputFormat::Human => println!(
            "Backup-set restore plan:\n{}",
            serde_json::to_string_pretty(&plan)?
        ),
    }
    Ok(())
}

fn print_job_human(job: &rocklake_catalog::JobRecord) {
    println!("Job {}:", job.id);
    println!("  Kind: {:?}", job.kind);
    println!("  State: {:?}", job.state);
    println!("  Resource class: {:?}", job.resource_class);
    println!("  Items: {}", job.progress.items_done);
    if let Some(total) = job.progress.items_total {
        println!("  Items total: {total}");
    }
    println!("  Bytes: {}", job.progress.bytes_done);
    println!("  Checkpoint: {}", job.checkpoint.sequence);
    if let Some(cursor) = &job.checkpoint.cursor {
        println!("  Cursor: {cursor}");
    }
    if let Some(error) = &job.error {
        println!("  Error: {}", error.message);
    }
}

async fn run_job<T, F, Fut>(
    db: &slatedb::Db,
    kind: rocklake_catalog::JobKind,
    parameters: serde_json::Value,
    idempotency_key: Option<String>,
    operation: F,
) -> Result<(T, rocklake_catalog::JobId), Box<dyn std::error::Error>>
where
    F: FnOnce(slatedb::Db) -> Fut,
    Fut: Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    let ledger = rocklake_catalog::JobLedger::new(db);
    let record = ledger
        .create(rocklake_catalog::JobRequest::new(
            kind,
            None,
            parameters,
            idempotency_key,
        ))
        .await?;
    let id = record.id.clone();
    ledger.start(&id).await?;
    match operation(db.clone()).await {
        Ok(result) => {
            ledger
                .complete(&id, serde_json::json!({"completed": true}))
                .await?;
            Ok((result, id))
        }
        Err(error) => {
            let _ = ledger
                .fail(
                    &id,
                    rocklake_catalog::JobError {
                        code: "operation_failed".to_string(),
                        message: error.to_string(),
                        retryable: true,
                    },
                )
                .await;
            Err(error)
        }
    }
}

// ─── serve ─────────────────────────────────────────────────────────────────

async fn cmd_catalogs(
    command: cli::CatalogsSubcommand,
    config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let loaded_config = config::load(config_path)?.1;
    match command {
        cli::CatalogsSubcommand::Validate { registry } => {
            let (_, file_config) = config::load(config_path)?;
            if let Some(location) = registry.or_else(|| {
                file_config
                    .registry
                    .as_ref()
                    .map(|registry| registry.location.clone())
            }) {
                let registry = open_registry(&location, &file_config).await?;
                registry.verify().await?;
                println!("Managed registry is valid.");
            } else {
                config::static_router(&file_config)?
                    .ok_or("no static router or managed registry configured")?;
                println!("Static router configuration is valid.");
            }
        }
        cli::CatalogsSubcommand::List { registry, output } => {
            let (_, file_config) = config::load(config_path)?;
            if let Some(location) = registry.or_else(|| {
                file_config
                    .registry
                    .as_ref()
                    .map(|registry| registry.location.clone())
            }) {
                let registry = open_registry(&location, &file_config).await?;
                let snapshot = registry.snapshot().await?;
                print_registry_rows(&snapshot, output);
            } else {
                let router = config::static_router(&file_config)?
                    .ok_or("no static router or managed registry configured")?;
                print_static_rows(&router, output);
            }
        }
        cli::CatalogsSubcommand::Status { registry, output } => {
            let (_, file_config) = config::load(config_path)?;
            if let Some(location) = registry.or_else(|| {
                file_config
                    .registry
                    .as_ref()
                    .map(|registry| registry.location.clone())
            }) {
                let registry = open_registry(&location, &file_config).await?;
                let status = registry.status().await?;
                match output {
                    cli::OutputFormat::Json => println!(
                        "{}",
                        serde_json::json!({
                            "router_mode": "registry",
                            "generation": status.generation,
                            "registry_format_version": status.format_version,
                            "catalogs": status.catalogs,
                            "routed_catalogs": status.routed_catalogs,
                            "next_audit_sequence": status.next_audit_sequence,
                        })
                    ),
                    cli::OutputFormat::Human => {
                        println!("Router mode       registry");
                        println!("Generation        {}", status.generation);
                        println!("Configured        {}", status.catalogs);
                        println!("Routed            {}", status.routed_catalogs);
                    }
                }
            } else {
                let router = config::static_router(&file_config)?
                    .ok_or("no static router or managed registry configured")?;
                let rows = static_catalog_rows(&router);
                match output {
                    cli::OutputFormat::Json => println!(
                        "{}",
                        serde_json::json!({
                            "router_mode": router.settings.mode,
                            "default_catalog": router.settings.default_catalog,
                            "max_open_catalogs": router.settings.max_open_catalogs,
                            "catalog_idle_timeout": router.settings.catalog_idle_timeout,
                            "open_handles": 0,
                            "catalogs": rows,
                        })
                    ),
                    cli::OutputFormat::Human => {
                        println!("Router mode       {}", router.settings.mode);
                        println!(
                            "Default catalog   {}",
                            router.settings.default_catalog.unwrap_or_default()
                        );
                        println!(
                            "Open handles      0 / {}",
                            router.settings.max_open_catalogs
                        );
                        println!("Configured        {}", router.catalogs.len());
                    }
                }
            }
        }
        cli::CatalogsSubcommand::Create(args) => {
            cmd_catalog_mutation(args, false, &loaded_config).await?
        }
        cli::CatalogsSubcommand::Register(args) => {
            cmd_catalog_mutation(args, true, &loaded_config).await?
        }
        cli::CatalogsSubcommand::Promote(args) => {
            let (_, file_config) = config::load(config_path)?;
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let id = args.id.parse()?;
            let node_id = args.node_id.parse()?;
            print_mutation(
                registry
                    .promote(
                        &id,
                        args.expected_generation,
                        &node_id,
                        &args.endpoint,
                        &args.request_id,
                    )
                    .await?,
            );
        }
        cli::CatalogsSubcommand::Activate(args) => {
            let (_, file_config) = config::load(config_path)?;
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let id = args.id.parse()?;
            let node_id = args.node_id.parse()?;
            print_mutation(
                registry
                    .activate_writer(
                        &id,
                        &node_id,
                        args.assignment_generation,
                        args.writer_epoch,
                        &args.request_id,
                    )
                    .await?,
            );
        }
        cli::CatalogsSubcommand::Rename(args) => {
            let (_, file_config) = config::load(config_path)?;
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let id = args.id.parse()?;
            print_mutation(registry.rename(&id, &args.alias, &args.request_id).await?);
        }
        cli::CatalogsSubcommand::SetMode(args) => {
            let (_, file_config) = config::load(config_path)?;
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let id = args.id.parse()?;
            print_mutation(
                registry
                    .set_mode(&id, catalog_mode(args.mode), &args.request_id)
                    .await?,
            );
        }
        cli::CatalogsSubcommand::Disable(args) => {
            let (_, file_config) = config::load(config_path)?;
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let id = args.id.parse()?;
            print_mutation(registry.disable(&id, &args.request_id).await?);
        }
        cli::CatalogsSubcommand::Enable(args) => {
            let (_, file_config) = config::load(config_path)?;
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let id = args.id.parse()?;
            print_mutation(registry.enable(&id, &args.request_id).await?);
        }
        cli::CatalogsSubcommand::Remove(args) => {
            let (_, file_config) = config::load(config_path)?;
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let id = args.id.parse()?;
            print_mutation(registry.remove(&id, &args.request_id).await?);
        }
    }
    Ok(())
}

async fn cmd_registry(
    command: cli::RegistrySubcommand,
    config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_, file_config) = config::load(config_path)?;
    match command {
        cli::RegistrySubcommand::Init(args) => {
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let snapshot = registry.init().await?;
            println!(
                "Registry initialized at generation {}.",
                snapshot.generation
            );
        }
        cli::RegistrySubcommand::Status(args) => {
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let status = registry.status().await?;
            print_registry_status(&status, args.output);
        }
        cli::RegistrySubcommand::Backup(args) => {
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let info = registry.backup(&args.output).await?;
            println!("Registry backup created: {}", info.path.display());
        }
        cli::RegistrySubcommand::Restore(args) => {
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let snapshot = registry.restore(&args.input).await?;
            println!("Registry restored at generation {}.", snapshot.generation);
        }
        cli::RegistrySubcommand::Verify(args) => {
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let status = registry.verify().await?;
            print_registry_status(&status, args.output);
        }
        cli::RegistrySubcommand::MigrateStatic(args) => {
            let location = configured_registry(args.registry, &file_config)?;
            let router = config::static_router(&file_config)?
                .ok_or("static router configuration is required for migration")?;
            let registry = open_registry(&location, &file_config).await?;
            registry.init().await?;
            print_mutation(registry.migrate_static(router, &args.request_id).await?);
        }
        cli::RegistrySubcommand::RegisterNode(args) => {
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let node_id: rocklake_router::NodeId = args.node_id.parse()?;
            let lease_id = args
                .lease_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let expires_at = unix_now_ms()?
                .checked_add(
                    args.lease_seconds
                        .checked_mul(1_000)
                        .ok_or("lease duration overflow")?,
                )
                .ok_or("lease expiry overflow")?;
            let lease = rocklake_router::NodeLease::new(
                node_id.to_string(),
                lease_id,
                args.endpoint,
                expires_at,
            )?;
            print_mutation(registry.register_node(lease, &args.request_id).await?);
        }
        cli::RegistrySubcommand::RenewNode(args) => {
            let location = configured_registry(args.registry, &file_config)?;
            let registry = open_registry(&location, &file_config).await?;
            let node_id: rocklake_router::NodeId = args.node_id.parse()?;
            let expires_at = unix_now_ms()?
                .checked_add(
                    args.lease_seconds
                        .checked_mul(1_000)
                        .ok_or("lease duration overflow")?,
                )
                .ok_or("lease expiry overflow")?;
            print_mutation(
                registry
                    .renew_node(&node_id, &args.lease_id, expires_at, &args.request_id)
                    .await?,
            );
        }
    }
    Ok(())
}

fn configured_registry(
    explicit: Option<String>,
    config: &config::ConfigFile,
) -> Result<String, Box<dyn std::error::Error>> {
    explicit
        .or_else(|| {
            config
                .registry
                .as_ref()
                .map(|registry| registry.location.clone())
        })
        .ok_or_else(|| "a registry location is required (use --registry or [registry])".into())
}

fn unix_now_ms() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64)
}

fn router_open_options(config: &config::ConfigFile) -> rocklake_router::RouterOpenOptions {
    rocklake_router::RouterOpenOptions {
        s3_endpoint: config.s3_endpoint.clone(),
        s3_path_style: config.s3_path_style.unwrap_or(false),
        ..rocklake_router::RouterOpenOptions::default()
    }
}

async fn open_registry(
    location: &str,
    config: &config::ConfigFile,
) -> Result<rocklake_router::CatalogRegistry, Box<dyn std::error::Error>> {
    Ok(
        rocklake_router::CatalogRegistry::open_location(location, &router_open_options(config))
            .await?,
    )
}

fn catalog_mode(mode: cli::CatalogModeArg) -> rocklake_router::CatalogMode {
    match mode {
        cli::CatalogModeArg::ReadWrite => rocklake_router::CatalogMode::ReadWrite,
        cli::CatalogModeArg::ReadOnly => rocklake_router::CatalogMode::ReadOnly,
    }
}

async fn cmd_catalog_mutation(
    args: cli::CatalogMutationArgs,
    register: bool,
    config: &config::ConfigFile,
) -> Result<(), Box<dyn std::error::Error>> {
    let location = configured_registry(args.registry, config)?;
    let registry = open_registry(&location, config).await?;
    let catalog = rocklake_router::CatalogConfig {
        id: args.id,
        aliases: args.aliases,
        catalog: args.catalog,
        data: args.data,
        mode: catalog_mode(args.mode),
        credential_provider: args.credential_provider,
        policy_reference: args.policy_reference,
        limits: rocklake_router::CatalogLimits::default(),
    };
    let mutation = if register {
        registry.register(catalog, &args.request_id).await?
    } else {
        registry.create(catalog, &args.request_id).await?
    };
    print_mutation(mutation);
    Ok(())
}

fn print_mutation(mutation: rocklake_router::RegistryMutation) {
    println!(
        "Registry mutation committed at generation {}{}{}.",
        mutation.generation,
        if mutation.replayed {
            " (idempotent replay)"
        } else {
            ""
        },
        mutation
            .writer_state
            .map(|state| format!("; writer state {state:?}"))
            .unwrap_or_default(),
    );
}

fn print_registry_status(status: &rocklake_router::RegistryStatus, output: cli::OutputFormat) {
    match output {
        cli::OutputFormat::Json => println!(
            "{}",
            serde_json::json!({
                "registry_format_version": status.format_version,
                "generation": status.generation,
                "catalogs": status.catalogs,
                "routed_catalogs": status.routed_catalogs,
                "next_audit_sequence": status.next_audit_sequence,
                "nodes": status.nodes,
                "assigned_writers": status.assigned_writers,
            })
        ),
        cli::OutputFormat::Human => {
            println!("Registry format   {}", status.format_version);
            println!("Generation        {}", status.generation);
            println!("Catalogs           {}", status.catalogs);
            println!("Routed            {}", status.routed_catalogs);
            println!("Next audit        {}", status.next_audit_sequence);
            println!("Nodes             {}", status.nodes);
            println!("Assigned writers  {}", status.assigned_writers);
        }
    }
}

fn static_catalog_rows(router: &rocklake_router::StaticConfig) -> Vec<serde_json::Value> {
    router
        .catalogs
        .iter()
        .map(|catalog| {
            serde_json::json!({
                "id": catalog.id.to_string(),
                "aliases": catalog.aliases.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "catalog": redact_router_location(&catalog.catalog),
                "data": redact_router_location(&catalog.data),
                "mode": catalog_mode_name(catalog.mode),
                "credential_provider": catalog.credential_provider.clone(),
                "policy_reference": catalog.policy_reference.clone(),
            })
        })
        .collect()
}

fn print_static_rows(router: &rocklake_router::StaticConfig, output: cli::OutputFormat) {
    let rows = static_catalog_rows(router);
    match output {
        cli::OutputFormat::Json => println!("{}", serde_json::json!({"catalogs": rows})),
        cli::OutputFormat::Human => {
            for row in rows {
                println!("{}  {}  {}", row["id"], row["aliases"], row["mode"]);
            }
        }
    }
}

fn print_registry_rows(snapshot: &rocklake_router::RegistrySnapshot, output: cli::OutputFormat) {
    let rows: Vec<_> = snapshot
        .catalogs
        .iter()
        .map(|catalog| {
            serde_json::json!({
                "id": catalog.id.to_string(),
                "aliases": catalog.aliases.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "catalog": redact_registry_location(&catalog.catalog),
                "data": redact_registry_location(&catalog.data),
                "mode": catalog_mode_name(catalog.mode),
                "lifecycle": lifecycle_name(catalog.lifecycle),
                "credential_provider": catalog.credential_provider.clone(),
                "policy_reference": catalog.policy_reference.clone(),
                "writer": catalog.writer_assignment.as_ref().map(|assignment| {
                    serde_json::json!({
                        "node_id": assignment.node_id.to_string(),
                        "endpoint": assignment.endpoint,
                        "assignment_generation": assignment.assignment_generation,
                        "state": format!("{:?}", assignment.state).to_ascii_lowercase(),
                        "writer_epoch": assignment.writer_epoch,
                    })
                }),
                "generation": catalog.updated_generation,
                "tombstone": catalog.tombstone,
            })
        })
        .collect();
    match output {
        cli::OutputFormat::Json => println!(
            "{}",
            serde_json::json!({
                "generation": snapshot.generation,
                "default_catalog": snapshot.default_catalog,
                "catalogs": rows,
            })
        ),
        cli::OutputFormat::Human => {
            for row in rows {
                println!(
                    "{}  {}  {}  {}",
                    row["id"], row["aliases"], row["mode"], row["lifecycle"]
                );
            }
        }
    }
}

fn redact_registry_location(value: &str) -> String {
    CatalogLocation::parse(value)
        .map(|location| redact_router_location(&location))
        .unwrap_or_else(|_| "[invalid]".into())
}

fn catalog_mode_name(mode: rocklake_router::CatalogMode) -> &'static str {
    match mode {
        rocklake_router::CatalogMode::ReadWrite => "read-write",
        rocklake_router::CatalogMode::ReadOnly => "read-only",
    }
}

fn lifecycle_name(lifecycle: rocklake_router::CatalogLifecycle) -> &'static str {
    match lifecycle {
        rocklake_router::CatalogLifecycle::Creating => "creating",
        rocklake_router::CatalogLifecycle::Active => "active",
        rocklake_router::CatalogLifecycle::ReadOnly => "read-only",
        rocklake_router::CatalogLifecycle::Disabled => "disabled",
        rocklake_router::CatalogLifecycle::Deleting => "deleting",
        rocklake_router::CatalogLifecycle::Deleted => "deleted",
        rocklake_router::CatalogLifecycle::Error => "error",
    }
}

fn redact_router_location(location: &rocklake_router::CatalogLocation) -> String {
    if location.scheme() == "file" {
        return "file://[redacted]".to_string();
    }
    format!(
        "{}://{}/[redacted]",
        location.scheme(),
        location.authority()
    )
}

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
    let registry_settings = file_config.registry.clone();
    let mut emergency_read_only = false;
    let mut registry_auth: Option<Arc<MultiPrincipalAuth>> = None;
    let managed_registry = if let Some(settings) = &registry_settings {
        match open_registry(&settings.location, &file_config).await {
            Ok(registry) => match registry.static_config().await {
                Ok(_) => {
                    let snapshot = registry.snapshot().await?;
                    if !snapshot.principals.is_empty() {
                        let principals =
                            rocklake_pgwire::PrincipalStore::from_records(&snapshot.principals)?;
                        registry_auth = Some(Arc::new(MultiPrincipalAuth {
                            principals: Arc::new(principals),
                            authorization: Arc::new(registry.authorization_policy().await?),
                        }));
                    }
                    Some(registry)
                }
                Err(_error) if settings.emergency_read_only => {
                    emergency_read_only = true;
                    None
                }
                Err(error) => return Err(error.into()),
            },
            Err(_error) if settings.emergency_read_only => {
                emergency_read_only = true;
                None
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    if registry_auth.is_none()
        && (!file_config.principals.is_empty() || !file_config.grants.is_empty())
    {
        let principals = rocklake_pgwire::PrincipalStore::from_records(&file_config.principals)?;
        let authorization = rocklake_router::AuthorizationPolicy::new(
            0,
            file_config.principals.clone(),
            file_config.grants.clone(),
        )?;
        registry_auth = Some(Arc::new(MultiPrincipalAuth {
            principals: Arc::new(principals),
            authorization: Arc::new(authorization),
        }));
    }
    let static_router = if let Some(registry) = &managed_registry {
        Some(registry.static_config().await?)
    } else if emergency_read_only {
        let settings = registry_settings
            .as_ref()
            .expect("emergency registry settings");
        let recovery_path = settings
            .recovery_file
            .as_deref()
            .ok_or("emergency_read_only requires registry.recovery_file")?;
        config::static_router(&config::load(Some(recovery_path))?.1)?
    } else {
        config::static_router(&file_config)?
    };
    if registry_auth.is_some() && static_router.is_none() {
        return Err("multi-principal authentication requires catalog routing".into());
    }
    let catalog_url = setting(
        args.catalog.or(args.path),
        "ROCKLAKE_CATALOG",
        file_config.catalog.clone(),
        None,
        "catalog",
    )?
    .or_else(|| {
        static_router
            .as_ref()
            .and_then(|router| {
                router.catalogs.iter().find(|catalog| {
                    catalog.aliases.iter().any(|alias| {
                        Some(alias.as_str()) == router.settings.default_catalog.as_deref()
                    })
                })
            })
            .map(|catalog| catalog.catalog.display_uri())
    })
    .ok_or("a catalog path is required (use `serve ./lake` or --catalog)")?;
    let mode = if emergency_read_only || args.read_only.unwrap_or(false) {
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
    let auth_verifier_file = setting(
        args.auth_verifier_file,
        "ROCKLAKE_AUTH_VERIFIER_FILE",
        file_config.auth_verifier_file.clone(),
        None,
        "auth verifier file",
    )?;
    let auth_verifier = auth_verifier_file
        .as_deref()
        .map(read_scram_verifier_file)
        .transpose()?;
    let mut auth_username = setting(
        args.auth_user,
        "ROCKLAKE_AUTH_USER",
        file_config.auth_user,
        None,
        "auth user",
    )?;
    let mut auth_password = auth_password;
    if auth_verifier.is_some() && auth_password.is_some() {
        return Err(
            "auth verifier file cannot be combined with auth_password or auth_password_file".into(),
        );
    }
    if let Some((verifier_username, verifier)) = auth_verifier {
        if auth_username
            .as_deref()
            .is_some_and(|username| username != verifier_username)
        {
            return Err("auth user does not match SCRAM verifier file".into());
        }
        auth_username = Some(verifier_username);
        auth_password = Some(verifier);
    }
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
        router: static_router,
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
        auth_username,
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
            Some(usize::MAX),
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
    if config.max_active_scans == 0
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
    let router_default = config.router.as_ref().map(|router| {
        router
            .settings
            .default_catalog
            .as_deref()
            .expect("validated router has a default catalog")
            .to_string()
    });
    let catalog_url = if let Some(router) = &config.router {
        let alias = router_default.as_deref();
        router
            .catalogs
            .iter()
            .find(|catalog| {
                catalog
                    .aliases
                    .iter()
                    .any(|candidate| Some(candidate.as_str()) == alias)
            })
            .map(|catalog| catalog.catalog.display_uri())
            .expect("validated router default exists")
    } else {
        config.catalog_url.clone()
    };
    let (catalog_path, object_store) =
        resolve_catalog_with_opts_mode(&catalog_url, &s3_opts, config.mode != "reader")?;

    let opts = OpenOptions {
        object_store: object_store.clone(),
        path: catalog_path,
        encryption: encryption.clone(),
    };

    let default_read_only = config.mode == "reader"
        || config.router.as_ref().is_some_and(|router| {
            router
                .catalogs
                .iter()
                .find(|catalog| catalog.catalog.display_uri() == catalog_url)
                .is_some_and(|catalog| catalog.mode == rocklake_router::CatalogMode::ReadOnly)
        });
    let store = if default_read_only {
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
        "Serving mode: {}, cost mode: {:?}",
        config.mode,
        config.cost_mode
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

    if let Some(router_config) = config.router {
        let router = rocklake_router::CatalogRouter::new(
            router_config,
            rocklake_router::RouterOpenOptions {
                s3_endpoint: s3_opts.endpoint,
                s3_path_style: s3_opts.path_style,
                encryption,
                force_read_only: config.mode == "reader",
            },
        );
        let route = router.resolve(router_default.as_deref())?;
        router.install_handle(route.id, catalog.clone()).await?;
        if let Some(multi_auth) = registry_auth {
            run_server_with_router_and_catalog_and_auth(
                server_config,
                router,
                catalog,
                access_mode,
                multi_auth,
            )
            .await?;
        } else {
            rocklake_pgwire::server::run_server_with_router_and_catalog(
                server_config,
                router,
                catalog,
                access_mode,
            )
            .await?;
        }
    } else {
        run_server_with_mode(server_config, catalog, access_mode).await?;
    }
    drop(managed_registry);
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

fn redact_catalog_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let scheme = &url[..scheme_end + 3];
        let rest = &url[scheme_end + 3..];
        if let Some(at_idx) = rest.find('@') {
            let after_at = &rest[at_idx..];
            return format!("{scheme}[redacted]{after_at}");
        }
    }
    url.to_string()
}

fn generate_duckdb_attach(config: &ServeConfig) -> String {
    let mut params = format!(
        "host={} port={} dbname=rocklake",
        config.bind_addr.ip(),
        config.bind_addr.port()
    );
    if let Some(user) = &config.auth_username {
        params.push_str(&format!(" user={user}"));
    }
    if config.tls_required {
        params.push_str(" sslmode=require");
    }
    format!("ATTACH 'ducklake:postgres:{params}' AS lake (DATA_PATH 'data');")
}

fn print_startup_summary(config: &ServeConfig, _store: &CatalogStore) {
    let tls = config.tls_cert.is_some() && config.tls_key.is_some();
    let auth = config.auth_username.is_some() && config.auth_password.is_some();
    let catalog_display = config
        .router
        .as_ref()
        .and_then(|router| {
            let default = router.settings.default_catalog.as_deref()?;
            router.catalogs.iter().find(|catalog| {
                catalog
                    .aliases
                    .iter()
                    .any(|alias| alias.as_str() == default)
            })
        })
        .map(|catalog| redact_router_location(&catalog.catalog))
        .unwrap_or_else(|| redact_catalog_url(&config.catalog_url));
    println!("RockLake {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Catalog       {catalog_display}");
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
    println!("{}", generate_duckdb_attach(config));
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

#[derive(serde::Deserialize)]
struct ScramVerifierFile {
    username: String,
    scram_verifier: String,
}

fn read_scram_verifier_file(path: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    const MAX_VERIFIER_FILE_BYTES: usize = 16 * 1024;
    let contents = std::fs::read(path)
        .map_err(|error| format!("failed to read auth verifier file {path}: {error}"))?;
    if contents.len() > MAX_VERIFIER_FILE_BYTES {
        return Err(format!("auth verifier file {path} is too large").into());
    }
    let record: ScramVerifierFile = serde_json::from_slice(&contents)
        .map_err(|error| format!("invalid auth verifier file {path}: {error}"))?;
    if record.username.is_empty() || record.username.len() > 63 {
        return Err(format!("invalid username in auth verifier file {path}").into());
    }
    let verifier = rocklake_pgwire::scram::ScramVerifier::decode(&record.scram_verifier)
        .ok_or_else(|| format!("invalid SCRAM verifier in auth verifier file {path}"))?;
    if verifier.iterations > 1_000_000 {
        return Err(
            format!("SCRAM iteration count is too high in auth verifier file {path}").into(),
        );
    }
    Ok((record.username, record.scram_verifier))
}

#[cfg(test)]
mod tests {
    use super::{read_scram_verifier_file, read_secret, redacted_config, setting, validate_config};
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
            catalog: Some("s3://user:secret@bucket/lake".to_string()),
            ..ConfigFile::default()
        };
        assert!(validate_config(&config).is_err());
        let redacted = redacted_config(&config).to_string();
        assert!(!redacted.contains("secret"));
        assert!(redacted.contains("s3://[redacted]@bucket/lake"));
    }

    #[test]
    fn verifier_file_is_parsed_without_exposing_a_password() {
        let file = tempfile::NamedTempFile::new().expect("create verifier file");
        let verifier = rocklake_pgwire::scram::ScramVerifier::from_password_with_salt(
            "secret",
            b"fixed-salt".to_vec(),
            4096,
        );
        std::fs::write(
            file.path(),
            serde_json::json!({
                "username": "admin",
                "scram_verifier": verifier.encode()
            })
            .to_string(),
        )
        .expect("write verifier file");
        let (username, encoded) =
            read_scram_verifier_file(file.path().to_str().unwrap()).expect("parse verifier file");
        assert_eq!(username, "admin");
        assert_eq!(encoded, verifier.encode());
        assert!(!encoded.contains("secret"));
    }

    #[test]
    fn config_example_omits_inert_limits() {
        let example = crate::config::example();
        assert!(!example.contains("stream_queue_depth"));
        assert!(!example.contains("max_buffered_rows"));
    }

    #[test]
    fn test_redact_catalog_url() {
        assert_eq!(
            super::redact_catalog_url("s3://AKIA:SECRET@my-bucket/lake"),
            "s3://[redacted]@my-bucket/lake"
        );
        assert_eq!(
            super::redact_catalog_url("postgres://user:pass@localhost:5432/lake"),
            "postgres://[redacted]@localhost:5432/lake"
        );
        assert_eq!(
            super::redact_catalog_url("file:///tmp/catalog"),
            "file:///tmp/catalog"
        );
    }

    #[test]
    fn test_generate_duckdb_attach() {
        let config_plain = super::ServeConfig {
            catalog_url: "file:///tmp/lake".to_string(),
            router: None,
            bind_addr: "127.0.0.1:5432".parse().unwrap(),
            max_sessions: 64,
            metrics_port: None,
            metrics_path: "/metrics".to_string(),
            tls_cert: None,
            tls_key: None,
            tls_required: false,
            auth_username: None,
            auth_password: None,
            mode: "writer".to_string(),
            cost_mode: rocklake_catalog::CostMode::default(),
            s3_endpoint: None,
            s3_path_style: false,
            encryption_key: None,
            extension_schemas: vec![],
            otlp_endpoint: None,
            idle_connection_timeout_secs: 60,
            drain_timeout_secs: 30,
            max_active_scans: 16,
            stream_queue_depth: 0,
            max_buffered_rows: 0,
            max_response_bytes: 67108864,
            slow_operation_threshold_ms: 1000,
        };
        assert_eq!(
            super::generate_duckdb_attach(&config_plain),
            "ATTACH 'ducklake:postgres:host=127.0.0.1 port=5432 dbname=rocklake' AS lake (DATA_PATH 'data');"
        );

        let mut config_auth_tls = config_plain.clone();
        config_auth_tls.auth_username = Some("ducklake_user".to_string());
        config_auth_tls.tls_required = true;
        assert_eq!(
            super::generate_duckdb_attach(&config_auth_tls),
            "ATTACH 'ducklake:postgres:host=127.0.0.1 port=5432 dbname=rocklake user=ducklake_user sslmode=require' AS lake (DATA_PATH 'data');"
        );
    }
}

#[derive(Clone)]
struct ServeConfig {
    catalog_url: String,
    router: Option<rocklake_router::StaticConfig>,
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
    max_active_scans: usize,
    stream_queue_depth: usize,
    max_buffered_rows: usize,
    max_response_bytes: usize,
    slow_operation_threshold_ms: u64,
}

// ─── gc ────────────────────────────────────────────────────────────────────

async fn cmd_gc(command: cli::GcSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_url, retention_days, apply, output, idempotency_key) = match command {
        cli::GcSubcommand::Plan(args) => {
            (args.catalog, args.retention_days, false, args.output, None)
        }
        cli::GcSubcommand::Apply(args) => (
            args.catalog,
            args.retention_days,
            true,
            args.output,
            args.idempotency_key,
        ),
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
        let proposed_retain_from = plan.proposed_retain_from;
        let (result, job_id) = run_job(
            &db,
            rocklake_catalog::JobKind::Retention,
            serde_json::json!({
                "catalog": catalog_url,
                "retention_days": retention_days,
                "proposed_retain_from": proposed_retain_from
            }),
            idempotency_key,
            |job_db| async move {
                rocklake_catalog::gc::gc_apply(&job_db, proposed_retain_from)
                    .await
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            },
        )
        .await?;
        match output {
            cli::OutputFormat::Json => println!(
                "{}",
                serde_json::json!({
                    "schema_version": 1,
                    "previous_retain_from": result.previous_retain_from,
                    "new_retain_from": result.new_retain_from,
                    "snapshots_hidden": result.snapshots_hidden,
                    "job_id": job_id
                })
            ),
            cli::OutputFormat::Human => {
                println!("GC Applied:");
                println!("  Previous retain-from: {}", result.previous_retain_from);
                println!("  New retain-from: {}", result.new_retain_from);
                println!("  Snapshots hidden: {}", result.snapshots_hidden);
                println!("  Job: {job_id}");
            }
        }
    }

    db.close().await?;
    Ok(())
}

// ─── excise ────────────────────────────────────────────────────────────────

async fn cmd_excise(command: cli::ExciseSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_url, before, apply, output, idempotency_key) = match command {
        cli::ExciseSubcommand::Plan(args) => (args.catalog, args.before, false, args.output, None),
        cli::ExciseSubcommand::Apply(args) => (
            args.catalog,
            args.before,
            true,
            args.output,
            args.idempotency_key,
        ),
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
        let (result, job_id) = run_job(
            &db,
            rocklake_catalog::JobKind::Excision,
            serde_json::json!({"catalog": catalog_url, "before_snapshot": before}),
            idempotency_key,
            |job_db| async move {
                rocklake_catalog::excise::excise_apply(&job_db, before, "operator")
                    .await
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            },
        )
        .await?;
        match output {
            cli::OutputFormat::Json => println!(
                "{}",
                serde_json::json!({
                    "schema_version": 1,
                    "keys_deleted": result.keys_deleted,
                    "keys_failed": result.keys_failed,
                    "audit_entry_id": result.audit_entry_id,
                    "job_id": job_id
                })
            ),
            cli::OutputFormat::Human => {
                println!("Excise Applied:");
                println!("  Keys deleted: {}", result.keys_deleted);
                println!("  Keys failed: {}", result.keys_failed);
                println!("  Audit entry ID: {}", result.audit_entry_id);
                println!("  Job: {job_id}");
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
    let job_output_path = output_path.clone();
    let (result, job_id) = run_job(
        &db,
        rocklake_catalog::JobKind::Export,
        serde_json::json!({
            "catalog": args.catalog,
            "output": output_path.clone(),
            "snapshot_id": snapshot_id
        }),
        args.idempotency_key,
        |job_db| async move {
            let mut file = std::fs::File::create(&job_output_path)
                .map_err(|e| format!("Cannot create output file: {e}"))?;
            Ok(rocklake_catalog::export::export_catalog(&job_db, snapshot_id, &mut file).await?)
        },
    )
    .await?;
    println!("Export complete:");
    println!("  Rows exported: {}", result.rows_exported);
    println!("  Tables exported: {}", result.tables_exported);
    println!("  Output: {output_path}");
    println!("  Job: {job_id}");

    db.close().await?;
    Ok(())
}

// ─── import ────────────────────────────────────────────────────────────────

async fn cmd_import(args: cli::ImportArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let input_path = args.input;
    let (result, job_id) = run_job(
        &db,
        rocklake_catalog::JobKind::Import,
        serde_json::json!({"catalog": args.catalog, "input": input_path}),
        args.idempotency_key,
        |job_db| async move {
            let file = std::fs::File::open(&input_path)
                .map_err(|e| format!("Cannot open input file: {e}"))?;
            Ok(
                rocklake_catalog::export::import_catalog(&job_db, std::io::BufReader::new(file))
                    .await?,
            )
        },
    )
    .await?;
    println!("Import complete:");
    println!("  Rows imported: {}", result.rows_imported);
    println!("  Tables imported: {}", result.tables_imported);
    println!("  Job: {job_id}");

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

    let (count, job_id) = run_job(
        &db,
        rocklake_catalog::JobKind::Rebuild,
        serde_json::json!({"catalog": args.catalog, "data_root": data_path}),
        args.idempotency_key,
        |job_db| async move {
            Ok(rocklake_catalog::export::rebuild_catalog(&job_db, &data_paths).await?)
        },
    )
    .await?;
    println!("Rebuild complete: {count} files registered.");
    println!("Job: {job_id}");

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

async fn cmd_capacity(command: cli::CapacitySubcommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        cli::CapacitySubcommand::Report(args) => {
            let (catalog_path, object_store) = resolve_catalog(&args.catalog)?;
            let db = slatedb::Db::open(catalog_path, object_store).await?;
            let state = rocklake_catalog::inspect::inspect_snapshot(&db).await?;
            db.close().await?;

            let pricing = args
                .pricing_file
                .as_deref()
                .map(rocklake_catalog::PricingFile::read)
                .transpose()?;
            let report = rocklake_catalog::build_capacity_report(
                &state,
                &rocklake_catalog::CapacityInput {
                    read_ops_per_second: args.read_ops_per_second,
                    write_ops_per_second: args.write_ops_per_second,
                    list_ops_per_second: args.list_ops_per_second,
                    delete_ops_per_second: args.delete_ops_per_second,
                    read_bytes_per_second: args.read_bytes_per_second,
                    write_bytes_per_second: args.write_bytes_per_second,
                    catalog_bytes: args.catalog_bytes,
                    cache_size_mb: args.cache_size_mb,
                    max_sessions: args.max_sessions,
                    max_active_scans: args.max_active_scans,
                    evidence_profile: args.evidence_profile,
                },
                pricing.as_ref(),
            )?;

            match args.output {
                cli::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                cli::OutputFormat::Human => report.print(),
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
        cli::VerifySubcommand::Catalog(args) => {
            let (result, job_id) = run_job(
                &db,
                rocklake_catalog::JobKind::Verification,
                serde_json::json!({"catalog": args.catalog, "target": "catalog"}),
                args.idempotency_key,
                |job_db| async move {
                    let mut result = rocklake_catalog::verify::verify_catalog(&job_db).await?;
                    if let Err(error) = rocklake_catalog::verify_audit_chain(&job_db).await {
                        result.errors.push(error.to_string());
                    }
                    Ok(result)
                },
            )
            .await?;
            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "tables_checked": result.tables_checked,
                        "rows_checked": result.rows_checked,
                        "errors": result.errors,
                        "warnings": result.warnings,
                        "ok": result.is_ok(),
                        "job_id": job_id
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
                    println!("  Job: {job_id}");
                }
            }
        }
        cli::VerifySubcommand::DataFiles(args) => {
            let (result, job_id) = run_job(
                &db,
                rocklake_catalog::JobKind::Verification,
                serde_json::json!({"catalog": args.catalog, "target": "data_files"}),
                args.idempotency_key,
                |job_db| async move {
                    Ok(
                        rocklake_catalog::cleanup::verify_data_files(&job_db, &object_store)
                            .await?,
                    )
                },
            )
            .await?;
            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "files_ok": result.files_ok,
                        "files_missing": result.files_missing,
                        "files_error": result.files_error,
                        "total_checked": result.total_checked,
                        "job_id": job_id
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
                    println!("  Job: {job_id}");
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
            let plan_for_job = plan.clone();
            let (result, job_id) = run_job(
                &db,
                rocklake_catalog::JobKind::Repair,
                serde_json::json!({"catalog": args.catalog, "action": "apply"}),
                args.idempotency_key,
                |job_db| async move {
                    Ok(rocklake_catalog::repair::repair_apply(&job_db, &plan_for_job).await?)
                },
            )
            .await?;
            match output {
                cli::OutputFormat::Json => println!(
                    "{}",
                    serde_json::json!({
                        "schema_version": 1,
                        "actions": plan.actions.iter().map(|action| format!("{action:?}")).collect::<Vec<_>>(),
                        "unrecoverable_errors": plan.unrecoverable_errors,
                        "applied": true,
                        "actions_applied": result.actions_applied,
                        "actions_failed": result.actions_failed,
                        "job_id": job_id
                    })
                ),
                cli::OutputFormat::Human => {
                    println!("Repair Applied:");
                    println!("  Actions applied: {}", result.actions_applied);
                    println!("  Actions failed: {}", result.actions_failed);
                    println!("  Job: {job_id}");
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
    let target_version = rocklake_core::tags::CATALOG_FORMAT_VERSION;
    let apply = args.apply;
    let dry_run = args.dry_run || !apply;

    if dry_run {
        let db = slatedb::Db::open(catalog_path, object_store).await?;
        let result = rocklake_catalog::migrate::migrate_dry_run(&db, target_version).await?;
        println!("Migration Dry Run:");
        println!("  Current version:    {}", result.current_version);
        println!("  Target version:     {}", result.target_version);
        println!("  Rows to migrate:    {}", result.rows_to_migrate);
        println!("  Estimated duration: ~{}s", result.estimated_seconds);
        println!("  Backup required:    {}", result.backup_required);
        println!("  Mode:               {:?}", result.mode);
        println!("  Rollback boundary:  {}", result.rollback_boundary);
        println!();
        println!("{}", result.description);
        if result.rows_to_migrate > 0 {
            println!();
            println!("Run with --apply to execute the migration.");
        }
        db.close().await?;
    } else {
        let catalog = CatalogStore::open(OpenOptions {
            object_store,
            path: catalog_path,
            encryption: None,
        })
        .await?;
        let (result, job_id) = run_job(
            catalog.db(),
            rocklake_catalog::JobKind::Migration,
            serde_json::json!({
                "source_format": rocklake_core::tags::CATALOG_FORMAT_VERSION,
                "target_format": target_version,
            }),
            None,
            |job_db| async move {
                rocklake_catalog::migrate::migrate_apply(&job_db, target_version, ".")
                    .await
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            },
        )
        .await?;
        println!("Migration Complete:");
        println!("  Rows migrated:  {}", result.rows_migrated);
        println!("  New version:    {}", result.new_version);
        println!("  Verified:        {}", result.verification_passed);
        println!("  Activated:       {}", result.activated);
        println!("  Job:             {}", job_id);
        if !result.backup_path.is_empty() {
            println!("  Backup written:  {}", result.backup_path);
        }
        catalog.close().await?;
    }
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
    let job_output_path = output_path.clone();

    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store).await?;

    let (result, job_id) = run_job(
        &db,
        rocklake_catalog::JobKind::Export,
        serde_json::json!({
            "catalog": catalog_url,
            "output": output_path.clone(),
            "snapshot_id": snapshot_id
        }),
        args.idempotency_key,
        |job_db| async move {
            let mut file = std::fs::File::create(&job_output_path)
                .map_err(|e| format!("Cannot create output file {job_output_path}: {e}"))?;
            Ok(rocklake_catalog::export::export_catalog(&job_db, snapshot_id, &mut file).await?)
        },
    )
    .await?;

    println!("Export complete (28 DuckLake spec + 4 extension catalog tables):");
    println!("  Rows exported:   {}", result.rows_exported);
    println!("  Tables exported: {}", result.tables_exported);
    println!("  Output:          {output_path}");
    println!("  Job:             {job_id}");

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
    if let Some(path) = setting(
        args.auth_verifier_file,
        "ROCKLAKE_AUTH_VERIFIER_FILE",
        file_config.auth_verifier_file,
        None,
        "auth verifier file",
    )?
    .as_deref()
    {
        match read_scram_verifier_file(path) {
            Ok((username, _)) if auth_user.as_deref().is_none_or(|user| user == username) => {
                doctor_check(
                    &mut checks,
                    "authentication verifier",
                    "pass",
                    format!("valid SCRAM verifier for {username}"),
                )
            }
            Ok(_) => doctor_check(
                &mut checks,
                "authentication verifier",
                "fail",
                "auth user does not match SCRAM verifier file",
            ),
            Err(error) => doctor_check(
                &mut checks,
                "authentication verifier",
                "fail",
                error.to_string(),
            ),
        }
    }
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

async fn cmd_status(
    args: cli::StatusArgs,
    config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_, file_config) = config::load(config_path)?;
    let catalog_url = setting(
        args.catalog.or(args.path),
        "ROCKLAKE_CATALOG",
        file_config.catalog.clone(),
        None,
        "catalog",
    )?
    .ok_or("a catalog path is required (use `status ./lake` or --catalog)")?;

    let json = collect_status(&catalog_url, &file_config).await?;
    match args.output {
        cli::OutputFormat::Human => {
            println!(
                "Catalog:       {}",
                json["catalog"].as_str().unwrap_or_default()
            );
            println!(
                "Status:        {}",
                json["status"].as_str().unwrap_or_default()
            );
            println!("Snapshot:      {}", json["snapshot_id"]);
            println!("Retain from:   {}", json["retain_from"]);
            println!(
                "Format:        {}",
                json["format_version"].as_str().unwrap_or_default()
            );
            println!("Schema ver:    {}", json["schema_version"]);
            for (label, key) in [
                ("Storage ver:", "catalog_storage"),
                ("Registry ver:", "registry"),
                ("Backup ver:", "backup_manifest"),
                ("Job ledger:", "job_ledger"),
                ("Audit schema:", "audit_schema"),
                ("JSON schema:", "public_json"),
                ("Evidence:", "evidence_schema"),
            ] {
                println!("{label:<15}{}", json["versions"][key]);
            }
        }
        cli::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&json)?),
    }
    Ok(())
}

fn version_json() -> serde_json::Value {
    serde_json::json!({
        "output_schema_version": rocklake_core::version::PUBLIC_JSON_SCHEMA_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "certified_sha": option_env!("ROCKLAKE_RELEASE_SHA").unwrap_or("unknown"),
        "target_triple": option_env!("ROCKLAKE_RELEASE_TARGET").unwrap_or("unknown"),
        "rust_version": option_env!("ROCKLAKE_RUST_VERSION").unwrap_or("unknown"),
        "catalog_read_format": rocklake_core::tags::CATALOG_FORMAT_VERSION,
        "catalog_write_format": rocklake_core::tags::CATALOG_FORMAT_VERSION,
        "versions": rocklake_core::version::CURRENT_VERSIONS,
        "build_provenance_available": option_env!("ROCKLAKE_PROVENANCE_AVAILABLE") == Some("true"),
    })
}

async fn collect_status(
    catalog_url: &str,
    file_config: &config::ConfigFile,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let s3_opts = S3Options {
        endpoint: file_config.s3_endpoint.clone(),
        path_style: file_config.s3_path_style.unwrap_or(false),
    };
    let (catalog_path, object_store) =
        resolve_catalog_with_opts_mode(catalog_url, &s3_opts, false)?;
    let store = CatalogStore::open_without_epoch(OpenOptions {
        object_store,
        path: catalog_path,
        encryption: None,
    })
    .await?;
    let json = serde_json::json!({
        "output_schema_version": rocklake_core::version::PUBLIC_JSON_SCHEMA_VERSION,
        "catalog": redact_catalog_url(catalog_url),
        "status": "ready",
        "snapshot_id": store.latest_committed_snapshot_id(),
        "retain_from": rocklake_catalog::gc::read_retain_from(store.db()).await?,
        "format_version": "DuckLake 1.0 (V1_0)",
        "schema_version": store.schema_version(),
        "versions": rocklake_core::version::CURRENT_VERSIONS,
        "compatibility": {
            "ducklake_catalog_read": [rocklake_core::version::DUCKLAKE_CATALOG_VERSION],
            "ducklake_catalog_write": [rocklake_core::version::DUCKLAKE_CATALOG_VERSION],
            "catalog_storage_read": [rocklake_core::version::CATALOG_STORAGE_VERSION],
            "catalog_storage_write": [rocklake_core::version::CATALOG_STORAGE_VERSION],
            "minimum_direct_upgrade": "v0.59.0",
            "downgrade_after_migration": "rejected_before_write"
        }
    });
    store.close().await?;
    Ok(json)
}

async fn cmd_support(
    command: cli::SupportSubcommand,
    config_path: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        cli::SupportSubcommand::Bundle(args) => {
            let (_, file_config) = config::load(config_path)?;
            std::fs::create_dir(&args.output).map_err(|error| {
                format!(
                    "cannot create support bundle directory {}: {error}",
                    args.output.display()
                )
            })?;

            let catalog_url = setting(
                args.catalog,
                "ROCKLAKE_CATALOG",
                file_config.catalog.clone(),
                None,
                "catalog",
            )?;
            write_bundle_json(&args.output, "version.json", &version_json())?;
            write_bundle_json(
                &args.output,
                "config.json",
                &serde_json::json!({
                    "schema_version": 1,
                    "config": redacted_config(&file_config),
                }),
            )?;

            let status = match catalog_url.as_deref() {
                Some(url) => match collect_status(url, &file_config).await {
                    Ok(status) => serde_json::json!({"available": true, "status": status}),
                    Err(error) => serde_json::json!({
                        "available": false,
                        "error": redact_catalog_url(&error.to_string()),
                    }),
                },
                None => serde_json::json!({
                    "available": false,
                    "error": "no catalog configured",
                }),
            };
            write_bundle_json(&args.output, "status.json", &status)?;

            let verification = match catalog_url.as_deref() {
                Some(url) => collect_verification(url, &file_config).await,
                None => serde_json::json!({
                    "available": false,
                    "error": "no catalog configured",
                }),
            };
            write_bundle_json(&args.output, "verification.json", &verification)?;

            let metrics_url = args.metrics_url.or_else(|| {
                file_config.metrics_port.map(|port| {
                    format!(
                        "http://127.0.0.1:{port}{}",
                        file_config.metrics_path.as_deref().unwrap_or("/metrics")
                    )
                })
            });
            let metrics = match metrics_url.as_deref() {
                Some(url) => fetch_metrics(url).await.unwrap_or_else(|error| {
                    format!(
                        "# metrics unavailable: {}\n",
                        redact_catalog_url(&error.to_string())
                    )
                }),
                None => "# metrics unavailable: no endpoint configured\n".to_string(),
            };
            std::fs::write(args.output.join("metrics.prom"), metrics)?;
            std::fs::write(
                args.output.join("logs.txt"),
                "RockLake writes text logs to stderr; no log files are managed by the binary.\n",
            )?;

            let manifest = serde_json::json!({
                "schema_version": 1,
                "release": env!("CARGO_PKG_VERSION"),
                "files": [
                    "version.json",
                    "config.json",
                    "status.json",
                    "verification.json",
                    "metrics.prom",
                    "logs.txt"
                ],
                "redaction": {
                    "secrets": "redacted",
                    "catalog_credentials": "redacted",
                    "logs": "not captured; RockLake logs to stderr"
                }
            });
            write_bundle_json(&args.output, "support-bundle.json", &manifest)?;
            println!("Support bundle written: {}", args.output.display());
        }
    }
    Ok(())
}

fn write_bundle_json(
    directory: &std::path::Path,
    name: &str,
    value: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(
        directory.join(name),
        serde_json::to_string_pretty(value)? + "\n",
    )?;
    Ok(())
}

async fn collect_verification(
    catalog_url: &str,
    file_config: &config::ConfigFile,
) -> serde_json::Value {
    let result = async {
        let s3_opts = S3Options {
            endpoint: file_config.s3_endpoint.clone(),
            path_style: file_config.s3_path_style.unwrap_or(false),
        };
        let (catalog_path, object_store) =
            resolve_catalog_with_opts_mode(catalog_url, &s3_opts, false)?;
        let store = CatalogStore::open_without_epoch(OpenOptions {
            object_store,
            path: catalog_path,
            encryption: None,
        })
        .await?;
        let verified = rocklake_catalog::verify::verify_catalog(store.db()).await?;
        let value = serde_json::json!({
            "available": true,
            "ok": verified.is_ok(),
            "tables_checked": verified.tables_checked,
            "rows_checked": verified.rows_checked,
            "errors": verified.errors,
            "warnings": verified.warnings,
        });
        store.close().await?;
        Ok::<_, Box<dyn std::error::Error>>(value)
    }
    .await;
    match result {
        Ok(value) => value,
        Err(error) => serde_json::json!({
            "available": false,
            "error": redact_catalog_url(&error.to_string()),
        }),
    }
}

async fn fetch_metrics(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let rest = url
        .strip_prefix("http://")
        .ok_or("support bundle metrics URL must use http://")?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, "metrics"));
    if authority.is_empty() {
        return Err("support bundle metrics URL has no host".into());
    }
    let path = format!("/{path}");
    let mut stream = tokio::net::TcpStream::connect(authority).await?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    let mut response = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count =
            tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut chunk))
                .await??;
        if count == 0 {
            break;
        }
        response.extend_from_slice(&chunk[..count]);
        if response.len() > 1024 * 1024 {
            return Err("metrics response exceeds 1 MiB".into());
        }
    }
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("invalid metrics HTTP response")?;
    let headers = std::str::from_utf8(&response[..header_end])?;
    if !headers
        .lines()
        .next()
        .is_some_and(|line| line.contains(" 200 "))
    {
        return Err(format!(
            "metrics endpoint returned {}",
            headers.lines().next().unwrap_or("unknown status")
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&response[header_end + 4..]).into_owned())
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
    if config.auth_verifier_file.is_some()
        && (config.auth_password.is_some() || config.auth_password_file.is_some())
    {
        return Err(
            "auth_verifier_file cannot be combined with auth_password or auth_password_file"
                .to_string(),
        );
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
    if let Some(path) = config.auth_verifier_file.as_deref() {
        read_scram_verifier_file(path).map_err(|error| error.to_string())?;
    }
    if let Some(registry) = &config.registry {
        CatalogLocation::parse(&registry.location)
            .map_err(|error| format!("invalid registry location: {error}"))?;
    }
    if !config.principals.is_empty() || !config.grants.is_empty() {
        rocklake_pgwire::PrincipalStore::from_records(&config.principals)?;
        rocklake_router::AuthorizationPolicy::new(
            0,
            config.principals.clone(),
            config.grants.clone(),
        )
        .map_err(|error| error.to_string())?;
    }
    config::static_router(config)?;
    Ok(())
}

fn redacted_config(config: &config::ConfigFile) -> serde_json::Value {
    serde_json::json!({
        "catalog": config.catalog.as_deref().map(redact_catalog_url),
        "router": config.router.as_ref().map(|_| "static"),
        "registry": config.registry.as_ref().map(|_| "managed"),
        "catalogs": config.catalogs.len(),
        "principals": config.principals.len(),
        "grants": config.grants.len(),
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
        "auth_verifier_file": config.auth_verifier_file,
        "mode": config.mode,
        "cost_mode": config.cost_mode,
        "s3_endpoint": config.s3_endpoint.as_deref().map(redact_catalog_url),
        "s3_path_style": config.s3_path_style,
        "encryption_key": config.encryption_key.as_ref().map(|_| "[redacted]"),
        "encryption_key_file": config.encryption_key_file,
        "extension_schemas": config.extension_schemas,
        "otlp_endpoint": config.otlp_endpoint.as_deref().map(redact_catalog_url),
        "idle_connection_timeout": config.idle_connection_timeout,
        "drain_timeout": config.drain_timeout,
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
            let output_path = args.out.clone();
            let catalog_identity = args.catalog.clone();
            let snapshot_id = args.snapshot_id;
            let (data_root, data_store) = if let Some(data_root) = &args.data_root {
                let (path, store) =
                    resolve_catalog_with_opts_mode(data_root, &S3Options::default(), false)?;
                (Some(path), Some(store))
            } else {
                (None, None)
            };
            let (info, job_id) = run_job(
                &db,
                rocklake_catalog::JobKind::Backup,
                serde_json::json!({
                    "catalog": catalog_identity,
                    "output": output_path,
                    "snapshot_id": snapshot_id,
                    "data_root": args.data_root,
                    "include_data": args.include_data,
                    "verify_data": args.verify_data
                }),
                args.idempotency_key,
                |job_db| async move {
                    rocklake_catalog::create_backup_with_options(
                        &job_db,
                        &output_path,
                        rocklake_catalog::BackupOptions {
                            source_identity: catalog_identity,
                            snapshot_id,
                            data_store,
                            data_root,
                            include_data_inventory: args.include_data,
                            verify_data: args.verify_data,
                            ..rocklake_catalog::BackupOptions::default()
                        },
                    )
                    .await
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
                },
            )
            .await?;
            db.close().await?;
            println!("Job: {job_id}");
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
    let overwrite_token = catalog_overwrite_token(&backup.manifest, &args.catalog);
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
            "overwrite_token": serde_json::Value::Null,
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
    let mut target_empty = true;
    while let Some(kv) = existing
        .next()
        .await
        .map_err(|e| format!("scan restore target: {e}"))?
    {
        if !kv
            .key
            .starts_with(&rocklake_core::keys::key_system(b"jobs:"))
        {
            target_empty = false;
            break;
        }
    }
    if !target_empty
        && apply
        && (!args.overwrite || args.overwrite_token.as_deref() != Some(overwrite_token.as_str()))
    {
        db.close().await?;
        return Err(format!(
            "restore target is not empty; run restore plan and pass --overwrite --overwrite-token {overwrite_token}"
        )
        .into());
    }
    let plan = serde_json::json!({
        "schema_version": 1,
        "backup": args.backup,
        "catalog": args.catalog,
        "snapshot_id": backup.manifest.snapshot_id,
        "rows": backup.manifest.row_count,
        "target_empty": target_empty,
        "overwrite_token": (!target_empty).then_some(&overwrite_token),
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
    let backup_snapshot = backup.manifest.snapshot_id;
    let catalog_identity = args.catalog.clone();
    let backup_path = args.backup.clone();
    let (result, job_id) = run_job(
        &db,
        rocklake_catalog::JobKind::Restore,
        serde_json::json!({
            "backup": backup_path,
            "catalog": catalog_identity,
            "overwrite": args.overwrite
        }),
        args.idempotency_key,
        |job_db| async move {
            let file = std::fs::File::open(data_path)?;
            if !target_empty {
                let mut delete_batch = slatedb::WriteBatch::new();
                let mut keys_deleted = 0usize;
                let mut keys = job_db.scan::<&[u8], _>(std::ops::RangeFull).await?;
                while let Some(kv) = keys
                    .next()
                    .await
                    .map_err(|e| format!("scan restore target for overwrite: {e}"))?
                {
                    if kv
                        .key
                        .starts_with(&rocklake_core::keys::key_system(b"jobs:"))
                    {
                        continue;
                    }
                    delete_batch.delete(&kv.key);
                    keys_deleted += 1;
                }
                if keys_deleted > 0 {
                    job_db.write(delete_batch).await?;
                }
            }
            let result = rocklake_catalog::export::import_catalog(
                    &job_db,
                std::io::BufReader::new(file),
            )
            .await?;
            let restored = rocklake_catalog::inspect::inspect_snapshot(&job_db).await?;
            if restored.latest_snapshot_id != backup_snapshot {
                return Err(format!(
                    "restore verification failed: restored snapshot {} differs from backup snapshot {}",
                    restored.latest_snapshot_id, backup_snapshot
                )
                .into());
            }
            Ok(result)
        },
    )
    .await?;
    db.close().await?;
    match args.output {
        cli::OutputFormat::Json => println!(
            "{}",
            serde_json::json!({
                "schema_version": 1,
                "restored": true,
                "rows_imported": result.rows_imported,
                "tables_imported": result.tables_imported,
                "verified": true,
                "job_id": job_id
            })
        ),
        cli::OutputFormat::Human => println!(
            "Restore applied: {} rows imported and verified (job {})",
            result.rows_imported, job_id
        ),
    }
    Ok(())
}

fn catalog_overwrite_token(manifest: &rocklake_catalog::BackupManifest, catalog: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(manifest.sha256.as_bytes());
    digest.update(b":");
    digest.update(catalog.as_bytes());
    let suffix = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("restore-{suffix}")
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
    let idempotency_key = args.idempotency_key;

    let (catalog_path, object_store) = resolve_catalog(&catalog_url)?;
    let db = slatedb::Db::open(catalog_path, object_store.clone()).await?;

    let config = rocklake_catalog::SweepOrphansConfig {
        grace_period_hours,
        apply,
        data_root: data_root.clone(),
    };

    let object_store_for_job = object_store.clone();
    let (result, job_id) = run_job(
        &db,
        rocklake_catalog::JobKind::OrphanSweep,
        serde_json::json!({
            "catalog": catalog_url,
            "data_root": data_root,
            "grace_period_hours": grace_period_hours,
            "apply": apply
        }),
        idempotency_key,
        |job_db| async move {
            Ok(rocklake_catalog::sweep_orphans(&job_db, object_store_for_job, &config).await?)
        },
    )
    .await?;
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
                "applied": apply,
                "job_id": job_id
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
            println!("  Job:                {job_id}");
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
