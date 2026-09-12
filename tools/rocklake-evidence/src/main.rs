//! Fresh-process scale evidence for the v0.52.0 LocalFS and MinIO matrix.
//!
//! This binary is internal tooling. It deliberately owns no benchmark data and
//! writes only the requested report paths. A `run` command prepares each
//! dataset and measures every operation in a separate child process.

use async_trait::async_trait;
use chrono::Utc;
use futures::TryStreamExt;
use object_store::path::Path as ObjectPath;
use object_store::{
    GetOptions, GetResult, ListResult, MultipartUpload, ObjectMeta, ObjectStore,
    PutMultipartOptions, PutOptions, PutPayload, PutResult,
};
use rocklake_catalog::{export::export_catalog, verify::verify_catalog};
use rocklake_catalog::{CatalogStore, OpenOptions, ReadOnlyCatalog};
use rocklake_core::mvcc::SnapshotId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const RELEASE: &str = "v0.52.0";
const SCHEMA_VERSION: u32 = rocklake_core::version::EVIDENCE_SCHEMA_VERSION;
const DEFAULT_SEED: u64 = 520;
const DEFAULT_BATCH_SIZE: usize = 1_000;
const DEFAULT_PAGE_SIZE: usize = 1_024;
const DEFAULT_SIZES: &[usize] = &[10_000, 100_000, 1_000_000];
const DEFAULT_OPERATIONS: &[&str] = &[
    "open",
    "schemas",
    "describe",
    "data-file-page",
    "data-file-stream",
    "data-file-materialized",
    "verify",
    "export",
];

type AnyResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone)]
struct Cli {
    command: String,
    backend: String,
    path: String,
    size: Option<usize>,
    sizes: Vec<usize>,
    seed: u64,
    batch_size: usize,
    page_size: usize,
    operation: Option<String>,
    operations: Vec<String>,
    table_id: Option<u64>,
    snapshot_id: Option<u64>,
    expected_digest: Option<String>,
    output: Option<PathBuf>,
    events: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DatasetMetadata {
    kind: String,
    release: String,
    schema_version: u32,
    backend: String,
    size: usize,
    seed: u64,
    batch_size: usize,
    table_id: u64,
    snapshot_id: u64,
    expected_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MachineMetadata {
    git_sha: String,
    binary_sha256: Option<String>,
    os: String,
    arch: String,
    kernel: Option<String>,
    cpu_count: usize,
    memory_bytes: Option<u64>,
    rust_version: String,
    container_image: Option<String>,
    dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Measurement {
    kind: String,
    release: String,
    schema_version: u32,
    backend: String,
    backend_version: String,
    dataset_size: usize,
    dataset_seed: u64,
    table_id: u64,
    snapshot_id: u64,
    expected_digest: String,
    operation: String,
    rows: u64,
    bytes: u64,
    observed_digest: Option<String>,
    matches_expected_digest: Option<bool>,
    open_us: u64,
    first_row_us: Option<u64>,
    operation_us: u64,
    close_us: u64,
    wall_us: u64,
    cpu_us: Option<u64>,
    baseline_rss_bytes: Option<u64>,
    peak_rss_bytes: Option<u64>,
    post_close_rss_bytes: Option<u64>,
    object_store_operations: u64,
    object_store_bytes: u64,
    machine: MachineMetadata,
    configuration: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct EvidenceReport {
    kind: String,
    release: String,
    schema_version: u32,
    generated_at: String,
    backend: String,
    dataset_sizes: Vec<usize>,
    operations: Vec<String>,
    machine: MachineMetadata,
    results: Vec<Measurement>,
}

#[derive(Debug)]
struct StoreStats {
    operations: AtomicU64,
    bytes: AtomicU64,
}

impl StoreStats {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            operations: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        })
    }
}

#[derive(Debug)]
struct CountingStore {
    inner: Arc<dyn ObjectStore>,
    stats: Arc<StoreStats>,
}

impl Display for CountingStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "counting({})", self.inner)
    }
}

#[async_trait]
impl ObjectStore for CountingStore {
    async fn put_opts(
        &self,
        location: &ObjectPath,
        payload: PutPayload,
        options: PutOptions,
    ) -> object_store::Result<PutResult> {
        self.stats.operations.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes
            .fetch_add(payload.content_length() as u64, Ordering::Relaxed);
        self.inner.put_opts(location, payload, options).await
    }

    async fn put_multipart_opts(
        &self,
        location: &ObjectPath,
        options: PutMultipartOptions,
    ) -> object_store::Result<Box<dyn MultipartUpload>> {
        self.stats.operations.fetch_add(1, Ordering::Relaxed);
        self.inner.put_multipart_opts(location, options).await
    }

    async fn get_opts(
        &self,
        location: &ObjectPath,
        options: GetOptions,
    ) -> object_store::Result<GetResult> {
        self.stats.operations.fetch_add(1, Ordering::Relaxed);
        let result = self.inner.get_opts(location, options).await?;
        self.stats
            .bytes
            .fetch_add(result.meta.size, Ordering::Relaxed);
        Ok(result)
    }

    async fn delete(&self, location: &ObjectPath) -> object_store::Result<()> {
        self.stats.operations.fetch_add(1, Ordering::Relaxed);
        self.inner.delete(location).await
    }

    fn list(
        &self,
        prefix: Option<&ObjectPath>,
    ) -> futures::stream::BoxStream<'static, object_store::Result<ObjectMeta>> {
        self.stats.operations.fetch_add(1, Ordering::Relaxed);
        self.inner.list(prefix)
    }

    async fn list_with_delimiter(
        &self,
        prefix: Option<&ObjectPath>,
    ) -> object_store::Result<ListResult> {
        self.stats.operations.fetch_add(1, Ordering::Relaxed);
        self.inner.list_with_delimiter(prefix).await
    }

    async fn copy(&self, from: &ObjectPath, to: &ObjectPath) -> object_store::Result<()> {
        self.stats.operations.fetch_add(1, Ordering::Relaxed);
        self.inner.copy(from, to).await
    }

    async fn copy_if_not_exists(
        &self,
        from: &ObjectPath,
        to: &ObjectPath,
    ) -> object_store::Result<()> {
        self.stats.operations.fetch_add(1, Ordering::Relaxed);
        self.inner.copy_if_not_exists(from, to).await
    }
}

struct Location {
    store: Arc<dyn ObjectStore>,
    catalog_path: ObjectPath,
    stats: Arc<StoreStats>,
}

impl Location {
    fn options(&self) -> OpenOptions {
        OpenOptions {
            object_store: Arc::clone(&self.store),
            path: self.catalog_path.clone(),
            encryption: None,
        }
    }
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    let cli = parse_cli()?;
    match cli.command.as_str() {
        "prepare" => prepare_dataset(&cli).await,
        "measure" => measure_operation(&cli).await.map(|measurement| {
            println!(
                "{}",
                serde_json::to_string(&measurement).expect("measurement JSON")
            );
        }),
        "run" => run_matrix(&cli).await,
        "help" | "--help" => {
            print_usage();
            Ok(())
        }
        command => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown command {command:?}; use `help`"),
        )
        .into()),
    }
}

fn parse_cli() -> AnyResult<Cli> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());
    let mut cli = Cli {
        command,
        backend: "localfs".to_string(),
        path: "benchmarks/evidence/v0.52.0/localfs".to_string(),
        size: None,
        sizes: DEFAULT_SIZES.to_vec(),
        seed: DEFAULT_SEED,
        batch_size: DEFAULT_BATCH_SIZE,
        page_size: DEFAULT_PAGE_SIZE,
        operation: None,
        operations: DEFAULT_OPERATIONS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        table_id: None,
        snapshot_id: None,
        expected_digest: None,
        output: None,
        events: None,
    };

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--backend" => cli.backend = next_value("--backend", &mut args)?,
            "--path" => cli.path = next_value("--path", &mut args)?,
            "--size" => cli.size = Some(next_value("--size", &mut args)?.parse()?),
            "--sizes" => cli.sizes = parse_sizes(&next_value("--sizes", &mut args)?)?,
            "--seed" => cli.seed = next_value("--seed", &mut args)?.parse()?,
            "--batch-size" => cli.batch_size = next_value("--batch-size", &mut args)?.parse()?,
            "--page-size" => cli.page_size = next_value("--page-size", &mut args)?.parse()?,
            "--operation" => cli.operation = Some(next_value("--operation", &mut args)?),
            "--operations" => {
                cli.operations = parse_operations(&next_value("--operations", &mut args)?)
            }
            "--table-id" => cli.table_id = Some(next_value("--table-id", &mut args)?.parse()?),
            "--snapshot-id" => {
                cli.snapshot_id = Some(next_value("--snapshot-id", &mut args)?.parse()?)
            }
            "--expected-digest" => {
                cli.expected_digest = Some(next_value("--expected-digest", &mut args)?)
            }
            "--output" => cli.output = Some(PathBuf::from(next_value("--output", &mut args)?)),
            "--events" => cli.events = Some(PathBuf::from(next_value("--events", &mut args)?)),
            "-h" | "--help" => {
                cli.command = "help".to_string();
            }
            unknown => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument {unknown:?}"),
                )
                .into())
            }
        }
    }
    if cli.batch_size == 0 || cli.page_size == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "batch and page sizes must be positive",
        )
        .into());
    }
    Ok(cli)
}

fn next_value<I: Iterator<Item = String>>(name: &str, args: &mut I) -> AnyResult<String> {
    args.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{name} needs a value")).into()
    })
}

fn parse_sizes(value: &str) -> AnyResult<Vec<usize>> {
    value
        .split(',')
        .map(|part| part.trim().parse::<usize>().map_err(Into::into))
        .collect()
}

fn parse_operations(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|part| part.trim().to_string())
        .collect()
}

async fn prepare_dataset(cli: &Cli) -> AnyResult<()> {
    let size = required(cli.size, "--size")?;
    if size == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "--size must be positive").into());
    }
    let location = make_location(&cli.path, &cli.backend)?;
    let mut catalog = CatalogStore::open(location.options()).await?;
    let mut created = 0usize;
    let mut table_id = 0u64;
    let mut latest_snapshot = 0u64;
    let mut digest = Sha256::new();

    while created < size {
        let mut writer = catalog.begin_write();
        if created == 0 {
            let schema_id = writer.create_schema("evidence").await?;
            table_id = writer.create_table(schema_id, "files", None).await?;
        }
        let end = (created + cli.batch_size).min(size);
        for index in created..end {
            let path = format!("data/seed-{}/part-{index:08}.parquet", cli.seed);
            let file_id = writer
                .register_data_file(table_id, &path, "parquet", 1, 4_096)
                .await?;
            digest_file(&mut digest, file_id, &path, 1, 4_096);
        }
        let commit = writer
            .create_snapshot(Some("v0.52.0-evidence"), Some("deterministic dataset"))
            .await?;
        latest_snapshot = commit.snapshot_id.as_u64();
        catalog.commit_writer(commit);
        created = end;
    }
    catalog.close().await?;

    let metadata = DatasetMetadata {
        kind: "dataset".to_string(),
        release: RELEASE.to_string(),
        schema_version: SCHEMA_VERSION,
        backend: cli.backend.clone(),
        size,
        seed: cli.seed,
        batch_size: cli.batch_size,
        table_id,
        snapshot_id: latest_snapshot,
        expected_digest: hex_digest(digest),
    };
    println!("{}", serde_json::to_string(&metadata)?);
    Ok(())
}

async fn measure_operation(cli: &Cli) -> AnyResult<Measurement> {
    let size = required(cli.size, "--size")?;
    let operation = required(cli.operation.clone(), "--operation")?;
    let table_id = required(cli.table_id, "--table-id")?;
    let snapshot_id = required(cli.snapshot_id, "--snapshot-id")?;
    let expected_digest = required(cli.expected_digest.clone(), "--expected-digest")?;
    let machine = machine_metadata();
    let backend_version = if cli.backend == "minio" {
        std::env::var("ROCKLAKE_EVIDENCE_MINIO_VERSION").unwrap_or_else(|_| "unknown".to_string())
    } else {
        "local-filesystem".to_string()
    };
    let sampler = RssSampler::start();
    let wall_start = Instant::now();
    let location = make_location(&cli.path, &cli.backend)?;
    let open_start = Instant::now();
    let mut read_only = None;
    let mut writerless = None;
    let open_us;
    if operation == "verify" || operation == "export" {
        writerless = Some(CatalogStore::open_without_epoch(location.options()).await?);
        open_us = micros(open_start.elapsed());
    } else {
        read_only = Some(ReadOnlyCatalog::open(location.options()).await?);
        open_us = micros(open_start.elapsed());
    }

    let operation_start = Instant::now();
    let operation_stats = match operation.as_str() {
        "open" => OperationStats::default(),
        "schemas" => {
            let reader = read_only
                .as_ref()
                .expect("read-only catalog for schemas")
                .read_at(SnapshotId::new(snapshot_id))?;
            OperationStats {
                rows: reader.list_schemas().await?.len() as u64,
                ..Default::default()
            }
        }
        "describe" => {
            let reader = read_only
                .as_ref()
                .expect("read-only catalog for describe")
                .read_at(SnapshotId::new(snapshot_id))?;
            let described = reader.describe_table(table_id).await?;
            OperationStats {
                rows: described.map_or(0, |(_, columns)| columns.len() as u64),
                ..Default::default()
            }
        }
        "data-file-page" => {
            let reader = read_only
                .as_ref()
                .expect("read-only catalog for pages")
                .read_at(SnapshotId::new(snapshot_id))?;
            consume_pages(&reader, table_id, cli.page_size, operation_start).await?
        }
        "data-file-stream" => {
            let reader = read_only
                .as_ref()
                .expect("read-only catalog for stream")
                .read_at(SnapshotId::new(snapshot_id))?;
            consume_stream(&reader, table_id, operation_start).await?
        }
        "data-file-materialized" => {
            let reader = read_only
                .as_ref()
                .expect("read-only catalog for materialized read")
                .read_at(SnapshotId::new(snapshot_id))?;
            let files = reader.list_data_files(table_id).await?;
            let mut stats = OperationStats::default();
            for file in files {
                stats.push_file(
                    file.data_file_id,
                    &file.path,
                    file.record_count,
                    file.file_size_bytes,
                    operation_start,
                );
            }
            stats
        }
        "verify" => {
            let catalog = writerless.as_ref().expect("catalog for verify");
            let result = verify_catalog(catalog.db()).await?;
            if !result.is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("verification failed: {:?}", result.errors),
                )
                .into());
            }
            OperationStats {
                rows: result.rows_checked,
                ..Default::default()
            }
        }
        "export" => {
            let catalog = writerless.as_ref().expect("catalog for export");
            let mut output = Vec::new();
            let result = export_catalog(catalog.db(), Some(snapshot_id), &mut output).await?;
            OperationStats {
                rows: result.rows_exported,
                bytes: output.len() as u64,
                ..Default::default()
            }
        }
        unknown => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown operation {unknown:?}"),
            )
            .into())
        }
    };
    let operation_us = micros(operation_start.elapsed());

    let close_start = Instant::now();
    if let Some(catalog) = read_only {
        catalog.close().await?;
    }
    if let Some(catalog) = writerless {
        catalog.close().await?;
    }
    let close_us = micros(close_start.elapsed());
    let wall_us = micros(wall_start.elapsed());
    let rss = sampler.finish();
    let observed_digest = operation_stats.digest.map(hex_digest);
    let matches_expected_digest = observed_digest
        .as_ref()
        .map(|digest| digest == &expected_digest);
    let stats = location.stats;

    let mut configuration = BTreeMap::new();
    configuration.insert("batch_size".to_string(), cli.batch_size.to_string());
    configuration.insert("page_size".to_string(), cli.page_size.to_string());
    configuration.insert("seed".to_string(), cli.seed.to_string());

    Ok(Measurement {
        kind: "measurement".to_string(),
        release: RELEASE.to_string(),
        schema_version: SCHEMA_VERSION,
        backend: cli.backend.clone(),
        backend_version,
        dataset_size: size,
        dataset_seed: cli.seed,
        table_id,
        snapshot_id,
        expected_digest,
        operation,
        rows: operation_stats.rows,
        bytes: operation_stats.bytes,
        observed_digest,
        matches_expected_digest,
        open_us,
        first_row_us: operation_stats.first_row_us,
        operation_us,
        close_us,
        wall_us,
        cpu_us: cpu_time_us(),
        baseline_rss_bytes: rss.baseline,
        peak_rss_bytes: rss.peak,
        post_close_rss_bytes: read_rss_bytes(),
        object_store_operations: stats.operations.load(Ordering::Relaxed),
        object_store_bytes: stats.bytes.load(Ordering::Relaxed),
        machine,
        configuration,
    })
}

#[derive(Default)]
struct OperationStats {
    rows: u64,
    bytes: u64,
    digest: Option<Sha256>,
    first_row_us: Option<u64>,
}

impl OperationStats {
    fn push_file(
        &mut self,
        id: u64,
        path: &str,
        record_count: u64,
        file_size_bytes: u64,
        start: Instant,
    ) {
        let digest = self.digest.get_or_insert_with(Sha256::new);
        digest_file(digest, id, path, record_count, file_size_bytes);
        self.rows += 1;
        self.bytes += file_size_bytes;
        self.first_row_us
            .get_or_insert_with(|| micros(start.elapsed()));
    }
}

async fn consume_pages(
    reader: &rocklake_catalog::CatalogReader,
    table_id: u64,
    page_size: usize,
    start: Instant,
) -> AnyResult<OperationStats> {
    let mut stats = OperationStats::default();
    let mut token = None;
    loop {
        let page = reader
            .list_data_files_paged(table_id, page_size, token.as_deref())
            .await?;
        for file in page.files {
            stats.push_file(
                file.data_file_id,
                &file.path,
                file.record_count,
                file.file_size_bytes,
                start,
            );
        }
        token = page.continuation_token;
        if token.is_none() {
            return Ok(stats);
        }
    }
}

async fn consume_stream(
    reader: &rocklake_catalog::CatalogReader,
    table_id: u64,
    start: Instant,
) -> AnyResult<OperationStats> {
    let mut stats = OperationStats::default();
    let mut stream = reader.stream_data_files(table_id).await?;
    while let Some(file) = stream.try_next().await? {
        stats.push_file(
            file.data_file_id,
            &file.path,
            file.record_count,
            file.file_size_bytes,
            start,
        );
    }
    Ok(stats)
}

async fn run_matrix(cli: &Cli) -> AnyResult<()> {
    let executable = std::env::current_exe()?;
    let mut results = Vec::new();
    for size in &cli.sizes {
        let dataset_path = dataset_path(&cli.path, &cli.backend, *size);
        let metadata = run_child(
            &executable,
            [
                "prepare".to_string(),
                "--backend".to_string(),
                cli.backend.clone(),
                "--path".to_string(),
                dataset_path.clone(),
                "--size".to_string(),
                size.to_string(),
                "--seed".to_string(),
                cli.seed.to_string(),
                "--batch-size".to_string(),
                cli.batch_size.to_string(),
            ],
        )?;
        let metadata: DatasetMetadata = serde_json::from_str(&metadata)?;
        for operation in &cli.operations {
            let measurement = run_child(
                &executable,
                [
                    "measure".to_string(),
                    "--backend".to_string(),
                    cli.backend.clone(),
                    "--path".to_string(),
                    dataset_path.clone(),
                    "--size".to_string(),
                    size.to_string(),
                    "--seed".to_string(),
                    cli.seed.to_string(),
                    "--batch-size".to_string(),
                    cli.batch_size.to_string(),
                    "--page-size".to_string(),
                    cli.page_size.to_string(),
                    "--operation".to_string(),
                    operation.clone(),
                    "--table-id".to_string(),
                    metadata.table_id.to_string(),
                    "--snapshot-id".to_string(),
                    metadata.snapshot_id.to_string(),
                    "--expected-digest".to_string(),
                    metadata.expected_digest.clone(),
                ],
            )?;
            results.push(serde_json::from_str::<Measurement>(&measurement)?);
        }
    }

    let output = cli.output.clone().unwrap_or_else(|| {
        PathBuf::from(format!("benchmarks/evidence/v0.52.0/{}.json", cli.backend))
    });
    let events = cli.events.clone().unwrap_or_else(|| {
        let mut path = output.clone();
        path.set_extension("jsonl");
        path
    });
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(parent) = events.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let report = EvidenceReport {
        kind: "evidence-report".to_string(),
        release: RELEASE.to_string(),
        schema_version: SCHEMA_VERSION,
        generated_at: Utc::now().to_rfc3339(),
        backend: cli.backend.clone(),
        dataset_sizes: cli.sizes.clone(),
        operations: cli.operations.clone(),
        machine: machine_metadata(),
        results: results.clone(),
    };
    std::fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    let mut lines = String::new();
    for result in results {
        lines.push_str(&serde_json::to_string(&result)?);
        lines.push('\n');
    }
    std::fs::write(&events, lines)?;
    println!("wrote {} and {}", output.display(), events.display());
    Ok(())
}

fn run_child<I>(executable: &Path, arguments: I) -> AnyResult<String>
where
    I: IntoIterator<Item = String>,
{
    let output = ProcessCommand::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "evidence child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn make_location(path: &str, backend: &str) -> AnyResult<Location> {
    let stats = StoreStats::new();
    let (inner, catalog_path): (Arc<dyn ObjectStore>, ObjectPath) = match backend {
        "local" | "localfs" => (
            {
                std::fs::create_dir_all(path)?;
                Arc::new(object_store::local::LocalFileSystem::new_with_prefix(path)?)
            },
            ObjectPath::from("catalog"),
        ),
        "minio" => {
            let (bucket, prefix) = parse_s3_path(path)?;
            let endpoint = std::env::var("ROCKLAKE_EVIDENCE_ENDPOINT").ok();
            let mut builder = object_store::aws::AmazonS3Builder::from_env()
                .with_bucket_name(bucket)
                .with_region("us-east-1")
                .with_virtual_hosted_style_request(false);
            if let Some(endpoint) = endpoint {
                builder = builder.with_endpoint(endpoint).with_allow_http(true);
            }
            (Arc::new(builder.build()?), ObjectPath::from(prefix))
        }
        unknown => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported backend {unknown:?}; use localfs or minio"),
            )
            .into())
        }
    };
    let counted: Arc<dyn ObjectStore> = Arc::new(CountingStore {
        inner,
        stats: Arc::clone(&stats),
    });
    Ok(Location {
        store: counted,
        catalog_path,
        stats,
    })
}

fn parse_s3_path(path: &str) -> AnyResult<(&str, &str)> {
    let rest = path.strip_prefix("s3://").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "MinIO path must use s3://bucket/prefix",
        )
    })?;
    let (bucket, prefix) = rest.split_once('/').unwrap_or((rest, ""));
    if bucket.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "MinIO bucket is empty").into());
    }
    Ok((bucket, prefix))
}

fn dataset_path(base: &str, backend: &str, size: usize) -> String {
    if backend == "minio" {
        format!("{}/{}", base.trim_end_matches('/'), size)
    } else {
        Path::new(base).join(size.to_string()).display().to_string()
    }
}

fn required<T>(value: Option<T>, name: &str) -> AnyResult<T> {
    value.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{name} is required")).into()
    })
}

fn digest_file(digest: &mut Sha256, id: u64, path: &str, record_count: u64, file_size_bytes: u64) {
    digest.update(id.to_le_bytes());
    digest.update(path.as_bytes());
    digest.update([0]);
    digest.update(record_count.to_le_bytes());
    digest.update(file_size_bytes.to_le_bytes());
}

fn hex_digest(digest: Sha256) -> String {
    Sha256::finalize(digest)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn micros(duration: Duration) -> u64 {
    duration.as_micros().min(u64::MAX as u128) as u64
}

struct RssSampler {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    baseline: Option<u64>,
    thread: JoinHandle<()>,
}

impl RssSampler {
    fn start() -> Self {
        let baseline = read_rss_bytes();
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(baseline.unwrap_or(0)));
        let thread_stop = Arc::clone(&stop);
        let thread_peak = Arc::clone(&peak);
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                if let Some(value) = read_rss_bytes() {
                    update_max(&thread_peak, value);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            if let Some(value) = read_rss_bytes() {
                update_max(&thread_peak, value);
            }
        });
        Self {
            stop,
            peak,
            baseline,
            thread,
        }
    }

    fn finish(self) -> RssSample {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.join().expect("RSS sampler thread");
        RssSample {
            baseline: self.baseline,
            peak: (self.peak.load(Ordering::Relaxed) > 0)
                .then(|| self.peak.load(Ordering::Relaxed)),
        }
    }
}

struct RssSample {
    baseline: Option<u64>,
    peak: Option<u64>,
}

fn update_max(value: &AtomicU64, candidate: u64) {
    value.fetch_max(candidate, Ordering::Relaxed);
}

#[cfg(target_os = "linux")]
fn read_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line
            .strip_prefix("VmRSS:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()?;
        Some(value * 1024)
    })
}

#[cfg(target_os = "macos")]
fn read_rss_bytes() -> Option<u64> {
    let output = ProcessCommand::new("ps")
        .args(["-o", "rss=", "-p"])
        .arg(std::process::id().to_string())
        .output()
        .ok()?;
    let kib = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(kib * 1024)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_rss_bytes() -> Option<u64> {
    None
}

fn machine_metadata() -> MachineMetadata {
    MachineMetadata {
        git_sha: command_text("git", &["rev-parse", "HEAD"])
            .unwrap_or_else(|| "unknown".to_string()),
        binary_sha256: std::env::current_exe().ok().and_then(|path| {
            std::fs::read(path).ok().map(|bytes| {
                let mut digest = Sha256::new();
                digest.update(bytes);
                hex_digest(digest)
            })
        }),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        kernel: command_text("uname", &["-sr"]),
        cpu_count: std::thread::available_parallelism().map_or(1, |count| count.get()),
        memory_bytes: memory_bytes(),
        rust_version: command_text("rustc", &["--version"])
            .unwrap_or_else(|| "unknown".to_string()),
        container_image: std::env::var("ROCKLAKE_EVIDENCE_CONTAINER_IMAGE").ok(),
        dependencies: BTreeMap::from([
            ("slatedb".to_string(), "0.13".to_string()),
            ("object_store".to_string(), "0.12".to_string()),
        ]),
    }
}

fn command_text(command: &str, args: &[&str]) -> Option<String> {
    let output = ProcessCommand::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "linux")]
fn memory_bytes() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    meminfo.lines().find_map(|line| {
        let value = line
            .strip_prefix("MemTotal:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()?;
        Some(value * 1024)
    })
}

#[cfg(unix)]
fn cpu_time_us() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    let user = timeval_us(usage.ru_utime)?;
    let system = timeval_us(usage.ru_stime)?;
    Some(user.saturating_add(system))
}

#[cfg(unix)]
fn timeval_us(value: libc::timeval) -> Option<u64> {
    if value.tv_sec < 0 || value.tv_usec < 0 {
        return None;
    }
    Some(
        (value.tv_sec as u64)
            .saturating_mul(1_000_000)
            .saturating_add(value.tv_usec as u64),
    )
}

#[cfg(not(unix))]
fn cpu_time_us() -> Option<u64> {
    None
}

#[cfg(target_os = "macos")]
fn memory_bytes() -> Option<u64> {
    command_text("sysctl", &["-n", "hw.memsize"]).and_then(|value| value.parse().ok())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn memory_bytes() -> Option<u64> {
    None
}

fn print_usage() {
    println!(
        "Usage:\n  rocklake-evidence prepare --backend localfs|minio --path PATH --size N\n  rocklake-evidence measure --backend ... --path PATH --size N --operation OP --table-id ID --snapshot-id ID --expected-digest HEX\n  rocklake-evidence run [--backend localfs|minio] [--path PATH] [--sizes 10000,100000,1000000] [--operations OP,...] [--output FILE] [--events FILE]\n\nOperations: open, schemas, describe, data-file-page, data-file-stream, data-file-materialized, verify, export\nMinIO uses ROCKLAKE_EVIDENCE_ENDPOINT plus the standard AWS credential environment variables."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_digest_is_deterministic() {
        let mut first = Sha256::new();
        let mut second = Sha256::new();
        for id in 1..=3 {
            let path = format!("data/part-{id}");
            digest_file(&mut first, id, &path, 1, 4_096);
            digest_file(&mut second, id, &path, 1, 4_096);
        }
        assert_eq!(hex_digest(first), hex_digest(second));
    }
}
