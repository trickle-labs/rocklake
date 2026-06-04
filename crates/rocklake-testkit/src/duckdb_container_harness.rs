//! DuckDB container harness for live tutorial-loop tests.

use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use futures::{Stream, StreamExt, TryStreamExt};
use tempfile::TempDir;
use testcontainers::bollard::{
    container::{
        AttachContainerOptions, Config, CreateContainerOptions, LogOutput, StartContainerOptions,
    },
    errors::Error as DockerError,
    image::CreateImageOptions,
    models::HostConfig,
    Docker,
};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

const DUCKDB_IMAGE: &str = "duckdb/duckdb";
const DUCKDB_TAG: &str = "1.5.3";
const DUCKDB_DATA_PATH: &str = "/duckdb-data";
const DUCKDB_HOME: &str = "/tmp";
const HOST_ALIAS: &str = "host.docker.internal";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(180);
const COMMAND_IDLE_TIMEOUT: Duration = Duration::from_secs(2);

static CONTAINER_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// Running DuckDB CLI container helper used for live container-loop tests.
pub struct DuckDbContainerHarness {
    container_name: String,
    session: Mutex<DuckDbSession>,
    attached: AtomicBool,
    _home_dir: TempDir,
    data_path: String,
}

struct DuckDbSession {
    input: Pin<Box<dyn tokio::io::AsyncWrite + Send>>,
    output: Pin<Box<dyn Stream<Item = Result<LogOutput, DockerError>> + Send>>,
}

/// Captured output from a DuckDB CLI invocation inside the container.
#[derive(Debug, Clone)]
pub struct DuckDbCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i64,
}

/// Errors returned by the DuckDB container harness.
#[derive(Debug, thiserror::Error)]
pub enum DuckDbContainerError {
    #[error("docker error: {0}")]
    Docker(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("utf-8 decoding failed: {0}")]
    Utf8(String),
    #[error("duckdb command failed (exit code {exit_code})\nstdout: {stdout}\nstderr: {stderr}")]
    CommandFailed {
        exit_code: i64,
        stdout: String,
        stderr: String,
    },
}

impl DuckDbContainerHarness {
    /// Start a DuckDB container harness with persistent home and data mounts.
    pub async fn start(data_dir: impl AsRef<Path>) -> Result<Self, DuckDbContainerError> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| DuckDbContainerError::Docker(e.to_string()))?;

        docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: DUCKDB_IMAGE,
                    from_src: "",
                    repo: "",
                    tag: DUCKDB_TAG,
                    platform: "",
                    changes: vec![],
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| DuckDbContainerError::Docker(e.to_string()))?;

        let host_data_dir = data_dir.as_ref().to_path_buf();
        let home_dir = TempDir::new().map_err(|e| DuckDbContainerError::Docker(e.to_string()))?;
        let container_name = format!(
            "rocklake-duckdb-{}-{}",
            std::process::id(),
            CONTAINER_COUNTER.fetch_add(1, Ordering::Relaxed)
        );

        let container = Config::<String> {
            image: Some(format!("{DUCKDB_IMAGE}:{DUCKDB_TAG}")),
            entrypoint: Some(vec!["duckdb".to_string()]),
            cmd: Some(vec!["-batch".to_string()]),
            env: Some(vec![format!("HOME={DUCKDB_HOME}")]),
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            open_stdin: Some(true),
            tty: Some(true),
            host_config: Some(HostConfig {
                binds: Some(vec![
                    format!("{}:{}:rw", host_data_dir.display(), DUCKDB_DATA_PATH),
                    format!("{}:{}:rw", home_dir.path().display(), DUCKDB_HOME),
                ]),
                extra_hosts: Some(vec![format!("{HOST_ALIAS}:host-gateway")]),
                auto_remove: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        let create_options = CreateContainerOptions {
            name: container_name.clone(),
            platform: None,
        };
        let created = docker
            .create_container(Some(create_options), container)
            .await
            .map_err(|e| DuckDbContainerError::Docker(e.to_string()))?;

        docker
            .start_container(&created.id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| DuckDbContainerError::Docker(e.to_string()))?;

        let attach = docker
            .attach_container(
                &created.id,
                Some(AttachContainerOptions::<String> {
                    stdin: Some(true),
                    stdout: Some(true),
                    stderr: Some(true),
                    stream: Some(true),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| DuckDbContainerError::Docker(e.to_string()))?;

        let mut session = DuckDbSession {
            input: attach.input,
            output: attach.output,
        };

        Self::drain_initial_output(&mut session).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        let harness = Self {
            container_name,
            session: Mutex::new(session),
            attached: AtomicBool::new(false),
            _home_dir: home_dir,
            data_path: DUCKDB_DATA_PATH.to_string(),
        };

        harness.install_ducklake_extension().await?;
        Ok(harness)
    }

    /// Container-side path where DuckDB writes tutorial data files.
    pub fn data_path(&self) -> &str {
        &self.data_path
    }

    /// Execute SQL in the running DuckDB CLI and capture stdout/stderr.
    pub async fn run_sql(&self, sql: &str) -> Result<DuckDbCommandOutput, DuckDbContainerError> {
        let original_sql = sql;
        let had_ducklake_attach =
            sql.contains("ATTACH 'ducklake:postgres:") && sql.contains("USE my_lake;");
        let attach_prelude = if had_ducklake_attach {
            extract_ducklake_attach_prelude(sql)
        } else {
            None
        };

        let mut sql = sql.to_string();
        let initial_detach_prepended = self.attached.load(Ordering::Relaxed) && had_ducklake_attach;
        if self.attached.load(Ordering::Relaxed) && had_ducklake_attach {
            sql = format!("USE memory; DETACH my_lake; {sql}");
        }

        let contains_row_mutation = {
            let upper = sql.to_ascii_uppercase();
            upper.contains("UPDATE ") || upper.contains("DELETE ")
        };

        if had_ducklake_attach && contains_row_mutation {
            let mut session = self.session.lock().await;
            if initial_detach_prepended {
                eprintln!("[duckdb harness] fast detach before wrapped mutation batch");
                let _ = Self::run_statement(&mut session, "USE memory").await?;
                let _ = Self::run_statement(&mut session, "DETACH my_lake").await?;
            }

            if let Some(attach_prelude) = &attach_prelude {
                for attach_statement in split_sql_statements(attach_prelude) {
                    let _ = Self::run_statement(&mut session, &attach_statement).await?;
                }
            }

            let body_sql = attach_prelude
                .as_ref()
                .and_then(|prelude| original_sql.get(prelude.len()..))
                .map(str::trim_start)
                .unwrap_or(original_sql);
            let wrapped_sql = format!("BEGIN; {}; COMMIT", body_sql.trim_end_matches(';'));
            let output = Self::run_statement(&mut session, &wrapped_sql).await?;

            let _ = Self::run_statement(&mut session, "CHECKPOINT").await?;

            eprintln!("[duckdb harness] checkpoint barrier");
            let _ = Self::run_statement(
                &mut session,
                "SELECT MAX(snapshot_id) FROM __ducklake_metadata_my_lake.ducklake_snapshot",
            )
            .await?;

            self.attached.store(true, Ordering::Relaxed);

            return Ok(DuckDbCommandOutput {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.exit_code,
            });
        }

        let mut session = self.session.lock().await;
        let statements = split_sql_statements(&sql);
        let mut combined_stdout = String::new();
        let mut combined_stderr = String::new();
        let mut exit_code = 0;
        let mut began_transaction = false;
        let mut checkpoint_after_commit = false;
        let mut refresh_after_batch = false;

        for (index, statement) in statements.iter().enumerate() {
            if !began_transaction && requires_explicit_transaction(statement) {
                eprintln!("[duckdb harness] begin");
                let _ = Self::run_statement(&mut session, "BEGIN").await?;
                began_transaction = true;
            }

            let statement_is_row_mutation = {
                let trimmed = statement.trim_start();
                let upper = trimmed.to_ascii_uppercase();
                upper.starts_with("UPDATE ") || upper.starts_with("DELETE ")
            };

            eprintln!("[duckdb harness] stmt={statement}");
            let output = Self::run_statement(&mut session, statement).await?;

            exit_code = output.exit_code;
            combined_stdout.push_str(&output.stdout);
            combined_stderr.push_str(&output.stderr);

            if requires_checkpoint(statement) {
                eprintln!("[duckdb harness] checkpoint");
                if began_transaction {
                    checkpoint_after_commit = true;
                } else {
                    let _ = Self::run_statement(&mut session, "CHECKPOINT").await?;
                }
            }

            let skip_refresh = initial_detach_prepended
                && index == 1
                && statement.eq_ignore_ascii_case("DETACH my_lake");

            let next_statement_is_mutation = statements
                .get(index + 1)
                .map(|next_statement| requires_refresh_before_next_statement(next_statement))
                .unwrap_or(false);

            if !skip_refresh
                && index + 1 < statements.len()
                && requires_refresh_before_next_statement(statement)
                && !next_statement_is_mutation
            {
                if began_transaction || statement_is_row_mutation {
                    refresh_after_batch = true;
                } else {
                    if checkpoint_after_commit {
                        let _ = Self::run_statement(&mut session, "CHECKPOINT").await?;
                        checkpoint_after_commit = false;
                    }
                    eprintln!("[duckdb harness] refresh");
                    if had_ducklake_attach {
                        eprintln!("[duckdb harness] checkpoint barrier");
                        let _ = Self::run_statement(
                            &mut session,
                            "SELECT MAX(snapshot_id) FROM __ducklake_metadata_my_lake.ducklake_snapshot",
                        )
                        .await?;
                    }
                    if let Some(attach_prelude) = &attach_prelude {
                        let _ = Self::run_statement(&mut session, "USE memory").await?;
                        let _ = Self::run_statement(&mut session, "DETACH my_lake").await?;
                        for attach_statement in split_sql_statements(attach_prelude) {
                            let _ = Self::run_statement(&mut session, &attach_statement).await?;
                        }
                    }
                }
            }
        }

        if began_transaction {
            if checkpoint_after_commit {
                let _ = Self::run_statement(&mut session, "CHECKPOINT").await?;
            }
            eprintln!("[duckdb harness] commit");
            let _ = Self::run_statement(&mut session, "COMMIT").await?;
        } else if checkpoint_after_commit {
            let _ = Self::run_statement(&mut session, "CHECKPOINT").await?;
        }

        if refresh_after_batch {
            eprintln!("[duckdb harness] refresh");
            if had_ducklake_attach {
                eprintln!("[duckdb harness] checkpoint barrier");
                let _ = Self::run_statement(
                    &mut session,
                    "SELECT MAX(snapshot_id) FROM __ducklake_metadata_my_lake.ducklake_snapshot",
                )
                .await?;
            }
            if let Some(attach_prelude) = &attach_prelude {
                let _ = Self::run_statement(&mut session, "USE memory").await?;
                let _ = Self::run_statement(&mut session, "DETACH my_lake").await?;
                for attach_statement in split_sql_statements(attach_prelude) {
                    let _ = Self::run_statement(&mut session, &attach_statement).await?;
                }
            }
        }

        if had_ducklake_attach {
            self.attached.store(true, Ordering::Relaxed);
        }

        Ok(DuckDbCommandOutput {
            stdout: combined_stdout,
            stderr: combined_stderr,
            exit_code,
        })
    }

    async fn install_ducklake_extension(&self) -> Result<(), DuckDbContainerError> {
        self.run_sql("LOAD ducklake;").await?;
        Ok(())
    }

    async fn run_statement(
        session: &mut DuckDbSession,
        statement: &str,
    ) -> Result<DuckDbCommandOutput, DuckDbContainerError> {
        let command = format!("{statement};\n");
        let DuckDbSession { input, output } = session;

        let write_future = async {
            input.write_all(command.as_bytes()).await?;
            input.flush().await
        };

        let read_future = Self::collect_command_output(output);

        let (write_result, output_result) = tokio::join!(
            tokio::time::timeout(COMMAND_TIMEOUT, write_future),
            tokio::time::timeout(COMMAND_TIMEOUT, read_future),
        );

        write_result
            .map_err(|_| DuckDbContainerError::Timeout("duckdb input write timed out".into()))?
            .map_err(|e| DuckDbContainerError::Docker(e.to_string()))?;

        output_result
            .map_err(|_| DuckDbContainerError::Timeout("duckdb output read timed out".into()))?
            .map_err(|e| DuckDbContainerError::Docker(e.to_string()))
    }

    async fn collect_command_output(
        output: &mut Pin<Box<dyn Stream<Item = Result<LogOutput, DockerError>> + Send>>,
    ) -> Result<DuckDbCommandOutput, DuckDbContainerError> {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let debug_frames = std::env::var_os("ROCKLAKE_DUCKDB_DEBUG").is_some();
        let mut saw_frame = false;

        loop {
            let next_frame = if saw_frame {
                match tokio::time::timeout(COMMAND_IDLE_TIMEOUT, output.next()).await {
                    Ok(frame) => frame,
                    Err(_) => break,
                }
            } else {
                tokio::time::timeout(COMMAND_TIMEOUT, output.next())
                    .await
                    .map_err(|_| {
                        DuckDbContainerError::Timeout("duckdb output read timed out".into())
                    })?
            };

            let Some(frame) = next_frame else {
                return Err(DuckDbContainerError::Docker(
                    "duckdb attach stream closed unexpectedly".into(),
                ));
            };

            let frame = frame.map_err(|e| DuckDbContainerError::Docker(e.to_string()))?;
            match frame {
                LogOutput::StdErr { message } => {
                    if debug_frames {
                        eprintln!("[duckdb frame] stderr {} bytes", message.len());
                    }
                    stderr.extend_from_slice(&message)
                }
                LogOutput::StdOut { message } | LogOutput::Console { message } => {
                    if debug_frames {
                        eprintln!("[duckdb frame] stdout/console {} bytes", message.len());
                    }
                    stdout.extend_from_slice(&message)
                }
                LogOutput::StdIn { message } => {
                    if debug_frames {
                        eprintln!("[duckdb frame] stdin {} bytes", message.len());
                    }
                }
            }

            saw_frame = true;
        }

        let stdout = String::from_utf8_lossy(&stdout).into_owned();
        let stderr = String::from_utf8_lossy(&stderr).into_owned();

        if looks_like_error(&stdout) || looks_like_error(&stderr) {
            return Err(DuckDbContainerError::CommandFailed {
                exit_code: -1,
                stdout,
                stderr,
            });
        }

        Ok(DuckDbCommandOutput {
            stdout,
            stderr,
            exit_code: 0,
        })
    }

    async fn drain_initial_output(session: &mut DuckDbSession) -> Result<(), DuckDbContainerError> {
        loop {
            let next_frame =
                tokio::time::timeout(Duration::from_secs(1), session.output.next()).await;

            match next_frame {
                Ok(Some(Ok(_frame))) => continue,
                Ok(Some(Err(e))) => {
                    return Err(DuckDbContainerError::Docker(e.to_string()));
                }
                Ok(None) => {
                    return Err(DuckDbContainerError::Docker(
                        "duckdb attach stream closed unexpectedly".into(),
                    ));
                }
                Err(_) => break,
            }
        }

        Ok(())
    }
}

impl Drop for DuckDbContainerHarness {
    fn drop(&mut self) {
        let _ = std::process::Command::new("docker")
            .args(["rm", "-f", &self.container_name])
            .status();
    }
}

fn looks_like_error(output: &str) -> bool {
    output.contains("Error:")
        || output.contains("Catalog Error:")
        || output.contains("Parser Error:")
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut chars = sql.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '\'' => {
                current.push(character);
                if in_single_quote {
                    if matches!(chars.peek(), Some('\'')) {
                        current.push(chars.next().unwrap());
                    } else {
                        in_single_quote = false;
                    }
                } else {
                    in_single_quote = true;
                }
            }
            ';' if !in_single_quote => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    statements.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(character),
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}

fn requires_checkpoint(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_ascii_uppercase();

    upper.starts_with("CREATE ")
        || upper.starts_with("INSERT ")
        || upper.starts_with("DROP ")
        || upper.starts_with("ALTER ")
        || upper.starts_with("ATTACH ")
        || upper.starts_with("DETACH ")
        || upper.starts_with("COPY ")
        || upper.starts_with("REPLACE ")
        || upper.starts_with("TRUNCATE ")
}

fn requires_refresh_before_next_statement(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_ascii_uppercase();

    upper.starts_with("INSERT ")
        || upper.starts_with("UPDATE ")
        || upper.starts_with("DELETE ")
        || upper.starts_with("CREATE ")
        || upper.starts_with("DROP ")
        || upper.starts_with("ALTER ")
        || upper.starts_with("ATTACH ")
        || upper.starts_with("DETACH ")
        || upper.starts_with("COPY ")
        || upper.starts_with("REPLACE ")
        || upper.starts_with("TRUNCATE ")
}

fn requires_explicit_transaction(statement: &str) -> bool {
    let trimmed = statement.trim_start();
    let upper = trimmed.to_ascii_uppercase();

    upper.starts_with("CREATE ")
        || upper.starts_with("INSERT ")
        || upper.starts_with("DROP ")
        || upper.starts_with("ALTER ")
        || upper.starts_with("REPLACE ")
        || upper.starts_with("TRUNCATE ")
}

fn extract_ducklake_attach_prelude(sql: &str) -> Option<String> {
    sql.find("USE my_lake;")
        .map(|position| sql[..position + "USE my_lake;".len()].to_string())
}
