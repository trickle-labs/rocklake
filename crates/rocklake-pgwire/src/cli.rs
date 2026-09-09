//! Clap-derived CLI structures for `rocklake`.
//!
//! This module defines every subcommand and flag using `clap` derive macros,
//! providing:
//! - Typed, validated argument parsing
//! - `--help` output for the top-level CLI and every subcommand (exit 0)
//! - Shell completion generation via `clap_complete`
//!
//! The module is deliberately kept separate from the command implementations
//! so that the structs can be unit-tested independently.

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

/// RockLake — serverless lakehouse catalog backed by SlateDB.
///
/// Run `rocklake <COMMAND> --help` for command-specific options.
#[derive(Debug, Parser)]
#[command(
    name = "rocklake",
    version,
    author,
    about = "RockLake: serverless lakehouse catalog backed by SlateDB",
    long_about = None,
)]
pub struct Cli {
    /// Optional TOML configuration file (defaults to ./rocklake.toml when present).
    #[arg(long, global = true, env = "ROCKLAKE_CONFIG")]
    pub config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the PG-Wire sidecar server.
    Serve(Box<ServeArgs>),

    /// Run a read-only startup preflight.
    Doctor(DoctorArgs),

    /// Check catalog and server readiness.
    Status(StatusArgs),

    /// Catalog lifecycle and maintenance operations.
    #[command(subcommand)]
    Catalog(CatalogSubcommand),

    /// Inspect and validate static multi-catalog routes.
    #[command(subcommand)]
    Catalogs(CatalogsSubcommand),

    /// Initialize and verify the managed catalog registry.
    #[command(subcommand)]
    Registry(RegistrySubcommand),

    /// Diagnostic and debugging operations.
    #[command(subcommand)]
    Debug(DebugSubcommand),

    /// Validate or print configuration.
    #[command(subcommand)]
    Config(ConfigSubcommand),

    /// Generate shell completion scripts.
    Completions(CompletionsArgs),

    // ─── Legacy top-level command aliases (hidden) ───────────────────────────
    /// Create and inspect portable catalog backups.
    #[command(subcommand, hide = true)]
    Backup(BackupSubcommand),

    /// Plan or apply a backup restore.
    #[command(subcommand, hide = true)]
    Restore(RestoreSubcommand),

    /// Visibility GC — advance the retain-from watermark.
    #[command(subcommand, hide = true)]
    Gc(GcSubcommand),

    /// Physical excision of catalog facts before a snapshot.
    #[command(subcommand, hide = true)]
    Excise(ExciseSubcommand),

    /// Manage catalog checkpoints.
    #[command(subcommand, hide = true)]
    Checkpoint(CheckpointSubcommand),

    /// Export catalog to NDJSON.
    #[command(hide = true)]
    Export(ExportArgs),

    /// Import catalog from NDJSON.
    #[command(hide = true)]
    Import(ImportArgs),

    /// Convert NDJSON export to PostgreSQL INSERT statements.
    #[command(name = "pg-migrate", hide = true)]
    PgMigrate(PgMigrateArgs),

    /// Rebuild catalog by scanning Parquet files in object storage.
    #[command(hide = true)]
    Rebuild(RebuildArgs),

    /// Inspect catalog state (snapshot, API costs, cache utilisation).
    #[command(subcommand, hide = true)]
    Inspect(InspectSubcommand),

    /// Verify catalog integrity.
    #[command(subcommand, hide = true)]
    Verify(VerifySubcommand),

    /// Repair catalog issues.
    #[command(hide = true)]
    Repair(RepairArgs),

    /// Warm up the block cache before serving.
    #[command(hide = true)]
    Warmup(WarmupArgs),

    /// Migrate catalog to the current format version.
    #[command(hide = true)]
    Migrate(MigrateArgs),

    /// Wire-corpus operations (diff and validate).
    #[command(subcommand, hide = true)]
    Corpus(CorpusSubcommand),

    /// Output recommended settings for a target cost.
    #[command(hide = true)]
    Tune(TuneArgs),

    /// Migrate from an existing DuckLake catalog into RockLake.
    #[command(name = "migrate-from-ducklake", hide = true)]
    MigrateFromDucklake(MigrateFromDucklakeArgs),

    /// Export all 28+ DuckLake catalog tables to NDJSON.
    #[command(name = "export-catalog", hide = true)]
    ExportCatalog(ExportCatalogArgs),

    /// Structured catalog health diagnostic report.
    #[command(hide = true)]
    Diagnose(DiagnoseArgs),

    /// Identify (and optionally delete) orphan Parquet files.
    #[command(name = "sweep-orphans", hide = true)]
    SweepOrphans(SweepOrphansArgs),
}

/// Options for `rocklake status`.
#[derive(Debug, Parser)]
pub struct StatusArgs {
    /// Catalog URL (`file:///…`, `s3://…`, `gs://…`, `az://…`).
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG", conflicts_with = "path")]
    pub catalog: Option<String>,

    /// Local catalog directory (equivalent to `--catalog ./lake`).
    #[arg(value_name = "PATH", conflicts_with = "catalog")]
    pub path: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,
}

/// Catalog lifecycle and maintenance operations.
#[derive(Debug, Subcommand)]
pub enum CatalogSubcommand {
    /// Create and inspect portable catalog backups.
    #[command(subcommand)]
    Backup(BackupSubcommand),

    /// Plan or apply a backup restore.
    #[command(subcommand)]
    Restore(RestoreSubcommand),

    /// Visibility GC — advance the retain-from watermark.
    #[command(subcommand)]
    Gc(GcSubcommand),

    /// Physical excision of catalog facts before a snapshot.
    #[command(subcommand)]
    Excise(ExciseSubcommand),

    /// Manage catalog checkpoints.
    #[command(subcommand)]
    Checkpoint(CheckpointSubcommand),

    /// Export catalog to NDJSON.
    Export(ExportArgs),

    /// Import catalog from NDJSON.
    Import(ImportArgs),

    /// Export all 28+ DuckLake catalog tables to NDJSON.
    #[command(name = "export-catalog")]
    ExportCatalog(ExportCatalogArgs),

    /// Migrate catalog to the current format version.
    Migrate(MigrateArgs),

    /// Verify catalog integrity.
    #[command(subcommand)]
    Verify(VerifySubcommand),

    /// Repair catalog issues.
    Repair(RepairArgs),

    /// Inspect and control durable administrative jobs.
    #[command(subcommand)]
    Jobs(JobSubcommand),
}

/// Static multi-catalog router operations.
#[derive(Debug, Subcommand)]
pub enum CatalogsSubcommand {
    /// Validate the static route configuration.
    Validate {
        /// Optional managed registry location.
        #[arg(long, env = "ROCKLAKE_REGISTRY")]
        registry: Option<String>,
    },
    /// List configured catalogs without opening them.
    List {
        /// Optional managed registry location.
        #[arg(long, env = "ROCKLAKE_REGISTRY")]
        registry: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        output: OutputFormat,
    },
    /// Show configured catalogs and current open-handle state.
    Status {
        /// Optional managed registry location.
        #[arg(long, env = "ROCKLAKE_REGISTRY")]
        registry: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        output: OutputFormat,
    },
    /// Create a new lazy catalog route.
    Create(CatalogMutationArgs),
    /// Register an existing catalog route.
    Register(CatalogMutationArgs),
    /// Rename a catalog route alias.
    Rename(CatalogRenameArgs),
    /// Change a catalog route mode.
    SetMode(CatalogSetModeArgs),
    /// Disable a catalog route without deleting bytes.
    Disable(CatalogIdArgs),
    /// Re-enable a disabled catalog route.
    Enable(CatalogIdArgs),
    /// Detach a route and retain a tombstone; never deletes bytes.
    Remove(CatalogIdArgs),
}

/// Managed registry operations.
#[derive(Debug, Subcommand)]
pub enum RegistrySubcommand {
    /// Initialize an empty registry.
    Init(RegistryLocationArgs),
    /// Show registry generation and lifecycle counts.
    Status(RegistryOutputArgs),
    /// Back up registry state and audit entries to a local directory.
    Backup(RegistryBackupArgs),
    /// Restore a backup into an uninitialized registry location.
    Restore(RegistryRestoreArgs),
    /// Verify registry state, indexes, prefixes, and audit sequence.
    Verify(RegistryOutputArgs),
    /// Import the v0.54 static route table from the selected config file.
    #[command(name = "migrate-static")]
    MigrateStatic(RegistryMigrateStaticArgs),
}

/// A registry location resolved from this flag, the environment, or config.
#[derive(Debug, Args)]
pub struct RegistryLocationArgs {
    /// Registry location (`file:///…`, `s3://…`, `gs://…`, `az://…`).
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
}

/// Registry output options.
#[derive(Debug, Args)]
pub struct RegistryOutputArgs {
    /// Registry location.
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,
}

/// Registry backup options.
#[derive(Debug, Args)]
pub struct RegistryBackupArgs {
    /// Registry location.
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
    /// Local output directory.
    #[arg(long)]
    pub output: std::path::PathBuf,
}

/// Registry restore options.
#[derive(Debug, Args)]
pub struct RegistryRestoreArgs {
    /// Target registry location.
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
    /// Local backup directory.
    #[arg(long)]
    pub input: std::path::PathBuf,
}

/// Static-to-registry migration options.
#[derive(Debug, Args)]
pub struct RegistryMigrateStaticArgs {
    /// Registry location.
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
    /// Caller-supplied idempotency key.
    #[arg(long, default_value = "migrate-static")]
    pub request_id: String,
}

/// Catalog definition used by create and register.
#[derive(Debug, Args)]
pub struct CatalogMutationArgs {
    /// Registry location.
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
    /// Stable UUID catalog identity.
    #[arg(long)]
    pub id: String,
    /// One or more PostgreSQL aliases.
    #[arg(long = "alias", required = true)]
    pub aliases: Vec<String>,
    /// SlateDB catalog location.
    #[arg(long)]
    pub catalog: String,
    /// Referenced data location.
    #[arg(long)]
    pub data: String,
    /// Access mode.
    #[arg(long, value_enum, default_value_t = CatalogModeArg::ReadWrite)]
    pub mode: CatalogModeArg,
    /// Named credential provider.
    #[arg(long)]
    pub credential_provider: String,
    /// Optional policy identifier.
    #[arg(long)]
    pub policy_reference: Option<String>,
    /// Caller-supplied idempotency key.
    #[arg(long)]
    pub request_id: String,
}

/// CLI catalog mode.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum CatalogModeArg {
    /// Read and write.
    #[default]
    ReadWrite,
    /// Read only.
    ReadOnly,
}

/// Catalog alias rename options.
#[derive(Debug, Args)]
pub struct CatalogRenameArgs {
    /// Registry location.
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
    /// Stable catalog UUID.
    #[arg(long)]
    pub id: String,
    /// New PostgreSQL alias.
    #[arg(long)]
    pub alias: String,
    /// Caller-supplied idempotency key.
    #[arg(long)]
    pub request_id: String,
}

/// Catalog mode options.
#[derive(Debug, Args)]
pub struct CatalogSetModeArgs {
    /// Registry location.
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
    /// Stable catalog UUID.
    #[arg(long)]
    pub id: String,
    /// New access mode.
    #[arg(long, value_enum)]
    pub mode: CatalogModeArg,
    /// Caller-supplied idempotency key.
    #[arg(long)]
    pub request_id: String,
}

/// Catalog ID mutation options.
#[derive(Debug, Args)]
pub struct CatalogIdArgs {
    /// Registry location.
    #[arg(long, env = "ROCKLAKE_REGISTRY")]
    pub registry: Option<String>,
    /// Stable catalog UUID.
    #[arg(long)]
    pub id: String,
    /// Caller-supplied idempotency key.
    #[arg(long)]
    pub request_id: String,
}

/// Durable administrative job controls.
#[derive(Debug, Subcommand)]
pub enum JobSubcommand {
    /// List jobs, newest update first.
    List(JobListArgs),
    /// Show one job and its last safe checkpoint.
    Status(JobStatusArgs),
    /// Request cancellation at the next safe checkpoint.
    Cancel(JobControlArgs),
    /// Explicitly requeue a failed, cancelled, or abandoned job.
    Resume(JobControlArgs),
}

/// Options for listing jobs.
#[derive(Debug, Parser)]
pub struct JobListArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

/// Options for showing one job.
#[derive(Debug, Parser)]
pub struct JobStatusArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Job UUID.
    #[arg(long)]
    pub id: String,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

/// Options for changing one job's lifecycle state.
#[derive(Debug, Parser)]
pub struct JobControlArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Job UUID.
    #[arg(long)]
    pub id: String,
}

/// Diagnostic and debugging operations.
#[derive(Debug, Subcommand)]
pub enum DebugSubcommand {
    /// Structured catalog health diagnostic report.
    Diagnose(DiagnoseArgs),

    /// Inspect catalog state (snapshot, API costs, cache utilisation).
    #[command(subcommand)]
    Inspect(InspectSubcommand),

    /// Wire-corpus operations (diff and validate).
    #[command(subcommand)]
    Corpus(CorpusSubcommand),

    /// Rebuild catalog by scanning Parquet files in object storage.
    Rebuild(RebuildArgs),

    /// Identify (and optionally delete) orphan Parquet files.
    #[command(name = "sweep-orphans")]
    SweepOrphans(SweepOrphansArgs),

    /// Convert NDJSON export to PostgreSQL INSERT statements.
    #[command(name = "pg-migrate")]
    PgMigrate(PgMigrateArgs),

    /// Output recommended settings for a target cost.
    Tune(TuneArgs),

    /// Warm up the block cache before serving.
    Warmup(WarmupArgs),

    /// Migrate from an existing DuckLake catalog into RockLake.
    #[command(name = "migrate-from-ducklake")]
    MigrateFromDucklake(MigrateFromDucklakeArgs),
}

// ─── serve ─────────────────────────────────────────────────────────────────

/// Options for `rocklake serve`.
#[derive(Debug, Parser)]
pub struct ServeArgs {
    /// Catalog URL (`file:///…`, `s3://…`, `gs://…`, `az://…`).
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG", conflicts_with = "path")]
    pub catalog: Option<String>,

    /// Local catalog directory (equivalent to `--catalog ./lake`).
    #[arg(value_name = "PATH", conflicts_with = "catalog")]
    pub path: Option<String>,

    /// Bind address for the PG-Wire listener.
    #[arg(short = 'b', long, help = "Bind address [default: 127.0.0.1:5432]")]
    pub bind: Option<String>,

    /// Maximum concurrent sessions.
    #[arg(long)]
    pub max_sessions: Option<usize>,

    /// Port for the Prometheus `/metrics` HTTP endpoint.
    #[arg(long)]
    pub metrics_port: Option<u16>,

    /// HTTP path for the metrics endpoint.
    #[arg(long)]
    pub metrics_path: Option<String>,

    /// Path to TLS certificate file.
    #[arg(long)]
    pub tls_cert: Option<String>,

    /// Path to TLS private key file.
    #[arg(long)]
    pub tls_key: Option<String>,

    /// Require TLS for all connections.
    #[arg(long, action = ArgAction::SetTrue)]
    pub tls_required: Option<bool>,

    /// Username for PG-Wire authentication.
    #[arg(long, env = "ROCKLAKE_AUTH_USER")]
    pub auth_user: Option<String>,

    /// Password for PG-Wire authentication.
    #[arg(long, hide_env_values = true, conflicts_with = "auth_password_file")]
    pub auth_password: Option<String>,

    /// Read the PG-Wire authentication password from a file.
    #[arg(long, conflicts_with = "auth_password")]
    pub auth_password_file: Option<String>,

    /// Serving mode: `writer` (accepts writes) or `reader` (read-only).
    #[arg(long, value_parser = ["writer", "reader"])]
    pub mode: Option<String>,

    /// Deprecated compatibility alias for `--mode reader`.
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "mode")]
    pub read_only: Option<bool>,

    /// Cost/latency preset.
    #[arg(long, value_parser = ["conservative", "balanced", "latency"])]
    pub cost_mode: Option<String>,

    /// S3-compatible endpoint URL (e.g. for MinIO).
    #[arg(long)]
    pub s3_endpoint: Option<String>,

    /// Use S3 path-style addressing.
    #[arg(long, action = ArgAction::SetTrue)]
    pub s3_path_style: Option<bool>,

    /// AES-256 encryption key (64 hex digits).
    #[arg(long, conflicts_with = "encryption_key_file", hide_env_values = true)]
    pub encryption_key: Option<String>,

    /// Read the AES-256 encryption key from a file.
    #[arg(
        long,
        env = "ROCKLAKE_ENCRYPTION_KEY_FILE",
        conflicts_with = "encryption_key"
    )]
    pub encryption_key_file: Option<String>,

    /// Comma-separated allowed extension schema names.
    #[arg(long, env = "ROCKLAKE_EXTENSION_SCHEMAS", value_delimiter = ',')]
    pub extension_schemas: Option<Vec<String>>,

    /// OTLP HTTP endpoint for OpenTelemetry tracing.
    #[arg(long, env = "ROCKLAKE_OTLP_ENDPOINT")]
    pub otlp_endpoint: Option<String>,

    /// Close idle connections after this many seconds (default: 60).
    #[arg(long)]
    pub idle_connection_timeout: Option<u64>,

    /// Maximum seconds to wait for in-flight queries during SIGTERM drain (default: 30).
    #[arg(long)]
    pub drain_timeout: Option<u64>,

    /// Capacity of the DataFusion AsyncBridge channel (default: 256).
    #[arg(long)]
    pub datafusion_bridge_queue_depth: Option<usize>,

    /// Maximum concurrent catalog scans (default: 25).
    #[arg(long)]
    pub max_active_scans: Option<usize>,
    /// Maximum queued stream items (default: 64).
    #[arg(long)]
    pub stream_queue_depth: Option<usize>,
    /// Maximum rows buffered for a response (default: 1024).
    #[arg(long)]
    pub max_buffered_rows: Option<usize>,
    /// Optional total response-byte policy; omitted means unlimited.
    #[arg(long)]
    pub max_response_bytes: Option<usize>,
    /// Log operations slower than this threshold in milliseconds (default: 1000).
    #[arg(long)]
    pub slow_operation_threshold_ms: Option<u64>,
}

#[derive(Debug, Parser)]
pub struct DoctorArgs {
    /// Catalog URL or local path to preflight.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Serving mode to validate.
    #[arg(long, value_parser = ["writer", "reader"])]
    pub mode: Option<String>,

    /// Listener address to validate for unsafe exposure.
    #[arg(long)]
    pub bind: Option<String>,

    /// TLS certificate path used by the intended server.
    #[arg(long)]
    pub tls_cert: Option<String>,

    /// TLS private key path used by the intended server.
    #[arg(long)]
    pub tls_key: Option<String>,

    /// Authentication username used by the intended server.
    #[arg(long, env = "ROCKLAKE_AUTH_USER")]
    pub auth_user: Option<String>,

    /// Encryption key or file to validate without printing its contents.
    #[arg(long, hide_env_values = true, conflicts_with = "encryption_key_file")]
    pub encryption_key: Option<String>,

    /// Encryption key file to validate.
    #[arg(long, conflicts_with = "encryption_key")]
    pub encryption_key_file: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

#[derive(Debug, Subcommand)]
pub enum ConfigSubcommand {
    /// Validate the selected configuration file.
    Check(ConfigCheckArgs),
    /// Print a complete example configuration.
    Example,
}

#[derive(Debug, Parser)]
pub struct ConfigCheckArgs {
    /// Configuration file to validate instead of the global --config path.
    #[arg(long)]
    pub file: Option<std::path::PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

#[derive(Debug, Subcommand)]
pub enum BackupSubcommand {
    /// Create a snapshot-consistent backup directory.
    Create(BackupCreateArgs),
    /// Validate and inspect a backup directory.
    Inspect(BackupInspectArgs),
}

#[derive(Debug, Parser)]
pub struct BackupCreateArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,
    /// Output backup directory.
    #[arg(long)]
    pub out: std::path::PathBuf,
    /// Snapshot to back up (latest by default).
    #[arg(long)]
    pub snapshot_id: Option<u64>,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Parser)]
pub struct BackupInspectArgs {
    /// Backup directory containing manifest.json and catalog.ndjson.
    pub backup: std::path::PathBuf,
    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

#[derive(Debug, Subcommand)]
pub enum RestoreSubcommand {
    /// Validate a backup and show its proposed target changes.
    Plan(RestoreArgs),
    /// Validate and import a backup into an empty target catalog.
    Apply(RestoreArgs),
}

#[derive(Debug, Parser)]
pub struct RestoreArgs {
    /// Backup directory containing manifest.json and catalog.ndjson.
    #[arg(long)]
    pub backup: std::path::PathBuf,
    /// Destination catalog URL or local path.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,
    /// Explicitly allow replacing a target catalog after validation.
    #[arg(long, action = ArgAction::SetTrue)]
    pub overwrite: bool,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

// ─── gc ────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum GcSubcommand {
    /// Show the GC plan without applying it.
    Plan(GcArgs),
    /// Apply the GC plan (advance retain-from).
    Apply(GcArgs),
}

#[derive(Debug, Parser)]
pub struct GcArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Retention period in days (snapshots older than this are eligible for GC).
    #[arg(long, default_value = "30")]
    pub retention_days: u64,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

// ─── excise ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum ExciseSubcommand {
    /// Show the excision plan without deleting anything.
    Plan(ExciseArgs),
    /// Apply the excision plan (physically delete old catalog facts).
    Apply(ExciseArgs),
}

#[derive(Debug, Parser)]
pub struct ExciseArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Delete facts for all snapshots strictly before this ID.
    #[arg(long)]
    pub before: u64,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

// ─── checkpoint ────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum CheckpointSubcommand {
    /// Create a new catalog checkpoint.
    Create(CheckpointCreateArgs),
    /// List existing checkpoints.
    List(CheckpointListArgs),
    /// Restore catalog to a saved checkpoint.
    Restore(CheckpointRestoreArgs),
    /// Pin a snapshot under a durable name.
    Pin(CheckpointPinArgs),
    /// Remove a named snapshot pin.
    Unpin(CheckpointUnpinArgs),
    /// List named snapshot pins.
    Pins(CheckpointListArgs),
}

#[derive(Debug, Parser)]
pub struct CheckpointCreateArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Human-readable label for the checkpoint.
    #[arg(long)]
    pub label: Option<String>,
}

#[derive(Debug, Parser)]
pub struct CheckpointListArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,
}

#[derive(Debug, Parser)]
pub struct CheckpointRestoreArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// ID of the checkpoint to restore.
    #[arg(long)]
    pub id: u64,
}

#[derive(Debug, Parser)]
pub struct CheckpointPinArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Name of the durable pin.
    #[arg(long)]
    pub name: String,

    /// Snapshot ID to retain.
    #[arg(long)]
    pub snapshot: u64,
}

#[derive(Debug, Parser)]
pub struct CheckpointUnpinArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Name of the durable pin.
    #[arg(long)]
    pub name: String,
}

// ─── export ────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct ExportArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Output file path.
    #[arg(long, default_value = "catalog.ndjson")]
    pub output: String,

    /// Export only this snapshot ID (default: latest).
    #[arg(long)]
    pub snapshot_id: Option<u64>,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

// ─── import ────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct ImportArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Input NDJSON file path.
    #[arg(long)]
    pub input: String,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

// ─── pg-migrate ────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct PgMigrateArgs {
    /// Input NDJSON export file.
    #[arg(long)]
    pub input: String,
}

// ─── rebuild ───────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct RebuildArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Object-store root containing the Parquet data files.
    #[arg(long)]
    pub data_root: Option<String>,

    /// S3-compatible endpoint URL.
    #[arg(long)]
    pub s3_endpoint: Option<String>,

    /// Use S3 path-style addressing.
    #[arg(long, action = ArgAction::SetTrue)]
    pub s3_path_style: bool,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

// ─── inspect ───────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum InspectSubcommand {
    /// Show the current snapshot metadata.
    Snapshot(InspectArgs),
    /// Show per-operation API cost estimates.
    #[command(name = "api-costs")]
    ApiCosts(InspectArgs),
    /// Show block-cache utilisation statistics.
    #[command(name = "cache-utilization")]
    CacheUtilization(InspectArgs),
}

#[derive(Debug, Parser)]
pub struct InspectArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

// ─── verify ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum VerifySubcommand {
    /// Verify catalog key-value integrity.
    Catalog(VerifyArgs),
    /// Verify that all registered data files are accessible.
    #[command(name = "data-files")]
    DataFiles(VerifyArgs),
}

#[derive(Debug, Parser)]
pub struct VerifyArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

// ─── repair ────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct RepairArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Preview repairs without applying them.
    #[arg(long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Apply repairs.
    #[arg(long, action = ArgAction::SetTrue)]
    pub apply: bool,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

// ─── warmup ────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct WarmupArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Number of tables to warm up (default: all).
    #[arg(long)]
    pub tables: Option<u64>,
}

// ─── migrate ───────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct MigrateArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Preview migration without writing.
    #[arg(long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Apply the migration.
    #[arg(long, action = ArgAction::SetTrue)]
    pub apply: bool,
}

// ─── corpus ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum CorpusSubcommand {
    /// Diff two wire-corpus files.
    Diff(CorpusDiffArgs),
    /// Validate a wire-corpus against the server.
    Validate(CorpusValidateArgs),
}

#[derive(Debug, Parser)]
pub struct CorpusDiffArgs {
    /// First corpus file.
    pub left: String,
    /// Second corpus file.
    pub right: String,
}

#[derive(Debug, Parser)]
pub struct CorpusValidateArgs {
    /// Corpus file to validate.
    pub corpus: String,
    /// Catalog URL to validate against.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,
}

// ─── tune ──────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct TuneArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Target monthly cost in USD.
    #[arg(long)]
    pub target_cost_usd: Option<f64>,
}

// ─── migrate-from-ducklake ─────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct MigrateFromDucklakeArgs {
    /// Source: `sqlite:/path/to/catalog.db` or path to an NDJSON dump.
    #[arg(long)]
    pub source: String,

    /// Destination RockLake catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Preview migration without writing.
    #[arg(long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Accept DuckLake catalog versions beyond the default (v1.0).
    /// May be specified multiple times.
    #[arg(long = "accept-version", action = ArgAction::Append)]
    pub accept_versions: Vec<String>,
}

// ─── export-catalog ────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct ExportCatalogArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Output NDJSON file path.
    #[arg(long, default_value = "catalog-export.ndjson")]
    pub out: String,

    /// Export only this snapshot ID (default: latest).
    #[arg(long)]
    pub at_snapshot: Option<u64>,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,
}

// ─── diagnose ──────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct DiagnoseArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Emit JSON output instead of human-readable text.
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,

    /// Output format. `--json` remains as a compatibility alias.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,

    /// Object-store root containing the data files (enables data-file checks).
    #[arg(long)]
    pub data_root: Option<String>,
}

// ─── sweep-orphans ─────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct SweepOrphansArgs {
    /// Catalog URL.
    #[arg(short = 'c', long, env = "ROCKLAKE_CATALOG")]
    pub catalog: String,

    /// Object-store prefix for data files (e.g. `s3://bucket/data/`).
    #[arg(long)]
    pub data_root: String,

    /// Grace period: files newer than this many hours are never deleted.
    #[arg(long, default_value = "24")]
    pub grace_period_hours: u64,

    /// Delete orphan files (default: dry-run only).
    #[arg(long, action = ArgAction::SetTrue)]
    pub apply: bool,
    /// Reuse the same job when retrying this operation.
    #[arg(long)]
    pub idempotency_key: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

// ─── completions ───────────────────────────────────────────────────────────

/// Generate shell completion scripts for `rocklake`.
///
/// Output the script to stdout and source it in your shell profile:
///
/// ```sh
/// # bash
/// rocklake completions bash >> ~/.bash_completion
/// # zsh
/// rocklake completions zsh > ~/.zfunc/_rocklake
/// # fish
/// rocklake completions fish > ~/.config/fish/completions/rocklake.fish
/// ```
#[derive(Debug, Parser)]
pub struct CompletionsArgs {
    /// Target shell.
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}
