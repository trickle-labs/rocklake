//! TCP server and configuration for the RockLake PG-Wire sidecar.
//!
//! Supports optional TLS (--tls-cert, --tls-key) and password authentication.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use bytes::BytesMut;
use pgwire::error::{ErrorInfo, PgWireError};
use pgwire::messages::PgWireBackendMessage;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::{watch, Mutex, Notify};
use tracing::{debug, error, info, info_span, warn, Instrument};

use rocklake_catalog::metrics::CatalogMetrics;
use rocklake_catalog::CatalogStore;

use crate::handler::RockLakeServerHandlers;
use crate::notify::NotifyManager;

/// Monotonically-tracked session counters used to populate Prometheus gauges.
#[derive(Default)]
pub struct SessionCounters {
    /// Current open connections.
    pub connections_open: AtomicI64,
    /// Open connections waiting for a query.
    pub connections_idle: AtomicI64,
    /// Queries currently executing.
    pub queries_in_flight: AtomicI64,
    /// Deprecated alias for `connections_open` kept for source compatibility.
    pub active_sessions: AtomicI64,
    /// Deprecated alias for `connections_idle` kept for source compatibility.
    pub idle_sessions: AtomicI64,
}

impl SessionCounters {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn publish(&self, metrics: Option<&Arc<CatalogMetrics>>) {
        let open = self.connections_open.load(Ordering::Relaxed).max(0) as u64;
        let idle = self.connections_idle.load(Ordering::Relaxed).max(0) as u64;
        let queries = self.queries_in_flight.load(Ordering::Relaxed).max(0) as u64;
        self.active_sessions.store(open as i64, Ordering::Relaxed);
        self.idle_sessions.store(idle as i64, Ordering::Relaxed);
        if let Some(metrics) = metrics {
            metrics.set_connections_open(open);
            metrics.set_connections_idle(idle);
            metrics.set_queries_in_flight(queries);
        }
    }
}

/// Shared state for one connection's activity and the server's drain state.
pub(crate) struct ConnectionActivity {
    counters: Arc<SessionCounters>,
    metrics: Option<Arc<CatalogMetrics>>,
    draining: Arc<AtomicBool>,
    query_in_flight: AtomicBool,
    query_idle_counted: AtomicBool,
    last_activity: StdMutex<Instant>,
    activity: Notify,
}

impl ConnectionActivity {
    pub(crate) fn new(
        counters: Arc<SessionCounters>,
        metrics: Option<Arc<CatalogMetrics>>,
        draining: Arc<AtomicBool>,
    ) -> Arc<Self> {
        Arc::new(Self {
            counters,
            metrics,
            draining,
            query_in_flight: AtomicBool::new(false),
            query_idle_counted: AtomicBool::new(false),
            last_activity: StdMutex::new(Instant::now()),
            activity: Notify::new(),
        })
    }

    pub(crate) fn standalone() -> Arc<Self> {
        Self::new(
            SessionCounters::new(),
            None,
            Arc::new(AtomicBool::new(false)),
        )
    }

    pub(crate) fn is_draining(&self) -> bool {
        self.draining.load(Ordering::Acquire)
    }

    pub(crate) fn begin_query(self: &Arc<Self>) -> QueryGuard {
        self.touch();
        if !self.query_in_flight.swap(true, Ordering::AcqRel) {
            let idle_counted = decrement_if_positive(&self.counters.connections_idle);
            self.query_idle_counted
                .store(idle_counted, Ordering::Release);
            self.counters
                .queries_in_flight
                .fetch_add(1, Ordering::AcqRel);
            self.counters.publish(self.metrics.as_ref());
        }
        QueryGuard {
            activity: self.clone(),
        }
    }

    fn finish_query(&self) {
        if self.query_in_flight.swap(false, Ordering::AcqRel) {
            if self.query_idle_counted.swap(false, Ordering::AcqRel) {
                self.counters
                    .connections_idle
                    .fetch_add(1, Ordering::AcqRel);
            }
            self.counters
                .queries_in_flight
                .fetch_sub(1, Ordering::AcqRel);
            self.counters.publish(self.metrics.as_ref());
        }
        self.touch();
    }

    pub(crate) fn touch(&self) {
        *self
            .last_activity
            .lock()
            .expect("connection activity mutex poisoned") = Instant::now();
        self.activity.notify_one();
    }

    fn idle_deadline(&self, timeout: Duration) -> Instant {
        *self
            .last_activity
            .lock()
            .expect("connection activity mutex poisoned")
            + timeout
    }

    fn idle_for_at_least(&self, timeout: Duration) -> bool {
        self.last_activity
            .lock()
            .expect("connection activity mutex poisoned")
            .elapsed()
            >= timeout
    }

    fn query_is_in_flight(&self) -> bool {
        self.query_in_flight.load(Ordering::Acquire)
    }
}

fn decrement_if_positive(counter: &AtomicI64) -> bool {
    loop {
        let current = counter.load(Ordering::Acquire);
        if current <= 0 {
            return false;
        }
        if counter
            .compare_exchange(current, current - 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return true;
        }
    }
}

pub(crate) struct QueryGuard {
    activity: Arc<ConnectionActivity>,
}

impl Drop for QueryGuard {
    fn drop(&mut self) {
        self.activity.finish_query();
    }
}

#[derive(Debug, Clone, Copy)]
enum ConnectionEnd {
    IdleTimeout,
    Shutdown,
}

async fn wait_for_connection_end(
    activity: Arc<ConnectionActivity>,
    idle_timeout: Duration,
    mut draining: watch::Receiver<bool>,
) -> ConnectionEnd {
    loop {
        if *draining.borrow() && !activity.query_is_in_flight() {
            return ConnectionEnd::Shutdown;
        }

        let activity_changed = activity.activity.notified();
        let draining_changed = draining.changed();
        if activity.query_is_in_flight() {
            tokio::select! {
                _ = activity_changed => {}
                result = draining_changed => {
                    if result.is_err() || *draining.borrow() && !activity.query_is_in_flight() {
                        return ConnectionEnd::Shutdown;
                    }
                }
            }
        } else {
            let deadline = activity.idle_deadline(idle_timeout);
            tokio::select! {
                _ = activity_changed => {}
                result = draining_changed => {
                    if (result.is_err() || *draining.borrow()) && !activity.query_is_in_flight() {
                        return ConnectionEnd::Shutdown;
                    }
                }
                _ = tokio::time::sleep_until(deadline.into()) => {
                    if !activity.query_is_in_flight() && activity.idle_for_at_least(idle_timeout) {
                        return ConnectionEnd::IdleTimeout;
                    }
                }
            }
        }
    }
}

async fn reject_connection(
    mut socket: tokio::net::TcpStream,
    code: &str,
    message: &str,
) -> std::io::Result<()> {
    let response = PgWireBackendMessage::ErrorResponse(
        ErrorInfo::new("FATAL".to_string(), code.to_string(), message.to_string()).into(),
    );
    let mut encoded = BytesMut::new();
    response
        .encode(&mut encoded)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    socket.write_all(&encoded).await?;
    socket.shutdown().await
}

pub(crate) fn server_shutting_down_error() -> PgWireError {
    PgWireError::UserError(Box::new(ErrorInfo::new(
        "FATAL".to_string(),
        "57P01".to_string(),
        "server is shutting down".to_string(),
    )))
}

/// TLS configuration.
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    /// Path to the TLS certificate file (PEM format).
    pub cert_path: Option<String>,
    /// Path to the TLS private key file (PEM format).
    pub key_path: Option<String>,
    /// Reject plaintext connections when TLS is not configured.
    /// Requires `cert_path` and `key_path` to be set.
    pub required: bool,
}

impl TlsConfig {
    pub fn is_enabled(&self) -> bool {
        self.cert_path.is_some() && self.key_path.is_some()
    }
}

/// Authentication configuration.
#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    /// Username for password authentication (None = no auth).
    pub username: Option<String>,
    /// Password for password authentication.
    pub password: Option<String>,
    /// Use SCRAM-SHA-256 instead of cleartext password authentication.
    ///
    /// When `true` the server initiates a SASL/SCRAM-SHA-256 exchange so
    /// that the plaintext credential is never transmitted over the wire.
    /// Requires `username` and `password` to be set.
    pub scram_sha256: bool,
}

impl AuthConfig {
    pub fn is_enabled(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }
}

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Bind address (default: 127.0.0.1:5432).
    pub bind_addr: SocketAddr,
    /// Maximum concurrent sessions (default: 50).
    pub max_sessions: usize,
    /// Maximum active scans (default: 25).
    pub max_active_scans: usize,
    pub stream_queue_depth: usize,
    pub max_buffered_rows: usize,
    pub max_response_bytes: usize,
    pub slow_operation_threshold: std::time::Duration,
    /// TLS configuration.
    pub tls: TlsConfig,
    /// Authentication configuration.
    pub auth: AuthConfig,
    /// Allowed extension schema names (default: `["pgtrickle"]`).
    pub extension_schemas: Vec<String>,
    /// Duration after which an idle connection is closed (default: 60 s).
    pub idle_connection_timeout: std::time::Duration,
    /// Grace period for in-flight queries during SIGTERM drain (default: 30 s).
    pub drain_timeout: std::time::Duration,
    /// Optional Prometheus metrics handle. When set, the server updates the
    /// connection lifecycle gauges and their deprecated session aliases.
    pub metrics: Option<Arc<CatalogMetrics>>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 5432)),
            max_sessions: 50,
            max_active_scans: 25,
            stream_queue_depth: 64,
            max_buffered_rows: 1024,
            max_response_bytes: 16 * 1024 * 1024,
            slow_operation_threshold: std::time::Duration::from_secs(1),
            tls: TlsConfig::default(),
            auth: AuthConfig::default(),
            extension_schemas: vec!["public".to_string(), "pgtrickle".to_string()],
            idle_connection_timeout: std::time::Duration::from_secs(60),
            drain_timeout: std::time::Duration::from_secs(30),
            metrics: None,
        }
    }
}

/// Build a TLS acceptor from cert and key paths.
fn build_tls_acceptor(tls_config: &TlsConfig) -> std::io::Result<Arc<tokio_rustls::TlsAcceptor>> {
    use std::io::BufReader;
    use tokio_rustls::rustls::{self, pki_types::PrivateKeyDer};

    // Ensure a crypto provider is installed (no-op if already set).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cert_path = tls_config.cert_path.as_ref().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "TLS cert path not configured",
        )
    })?;
    let key_path = tls_config.key_path.as_ref().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "TLS key path not configured",
        )
    })?;

    let cert_file = std::fs::File::open(cert_path)
        .map_err(|e| std::io::Error::other(format!("cannot open TLS cert: {e}")))?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<_> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<_, _>>()
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid cert: {e}"),
            )
        })?;

    let key_file = std::fs::File::open(key_path)
        .map_err(|e| std::io::Error::other(format!("cannot open TLS key: {e}")))?;
    let mut key_reader = BufReader::new(key_file);
    let key: PrivateKeyDer = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("invalid key: {e}"))
        })?
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "no private key found")
        })?;

    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("TLS config error: {e}"),
            )
        })?;

    Ok(Arc::new(tokio_rustls::TlsAcceptor::from(Arc::new(
        server_config,
    ))))
}

/// Run the RockLake PG-Wire server.
///
/// This function does not return until the process receives SIGTERM (Unix) or
/// a hard error on the listener.  On SIGTERM it stops accepting new connections
/// and waits up to `config.drain_timeout` for in-flight sessions to finish.
pub async fn run_server(
    config: ServerConfig,
    catalog: Arc<Mutex<CatalogStore>>,
) -> std::io::Result<()> {
    run_server_with_mode(config, catalog, crate::executor::AccessMode::Writer).await
}

/// Run the RockLake PG-Wire server with an immutable access mode.
pub async fn run_server_with_mode(
    config: ServerConfig,
    catalog: Arc<Mutex<CatalogStore>>,
    access_mode: crate::executor::AccessMode,
) -> std::io::Result<()> {
    #[cfg(unix)]
    let shutdown_signal = async {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        sigterm.recv().await;
    };
    #[cfg(not(unix))]
    let shutdown_signal = tokio::signal::ctrl_c();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = shutdown_signal.await;
        let _ = shutdown_tx.send(());
    });

    run_server_with_shutdown_mode(config, catalog, shutdown_rx, access_mode).await
}

/// Run the server with a shutdown signal (for testing and graceful drain).
pub async fn run_server_with_shutdown(
    config: ServerConfig,
    catalog: Arc<Mutex<CatalogStore>>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) -> std::io::Result<()> {
    run_server_with_shutdown_mode(
        config,
        catalog,
        shutdown,
        crate::executor::AccessMode::Writer,
    )
    .await
}

/// Run the server with a shutdown signal and an immutable access mode.
pub async fn run_server_with_shutdown_mode(
    config: ServerConfig,
    catalog: Arc<Mutex<CatalogStore>>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
    access_mode: crate::executor::AccessMode,
) -> std::io::Result<()> {
    let tls_acceptor = if config.tls.is_enabled() {
        Some(build_tls_acceptor(&config.tls)?)
    } else if config.tls.required {
        return Err(std::io::Error::other(
            "--tls-required is set but no TLS certificate/key were provided",
        ));
    } else {
        None
    };

    // Warn when authenticated connections are not protected by TLS. SCRAM
    // protects the password, while the explicit cleartext compatibility path
    // does not.
    if config.auth.is_enabled() && tls_acceptor.is_none() {
        if config.auth.scram_sha256 {
            warn!(
                "SCRAM-SHA-256 authentication is enabled without TLS. Use --tls-cert / \
                 --tls-key to protect the connection and server identity."
            );
        } else {
            warn!(
                "Cleartext password authentication is enabled without TLS. Credentials will \
                 be sent in plaintext. Use --tls-cert / --tls-key to enable TLS."
            );
        }
    }

    let listener = TcpListener::bind(config.bind_addr).await?;
    if tls_acceptor.is_some() {
        info!("RockLake serving on {} (TLS enabled)", config.bind_addr);
    } else {
        info!("RockLake serving on {}", config.bind_addr);
    }

    let session_semaphore = Arc::new(tokio::sync::Semaphore::new(config.max_sessions));
    let scan_semaphore = Arc::new(tokio::sync::Semaphore::new(config.max_active_scans));
    let auth_config = Arc::new(config.auth);
    let tls_required = config.tls.required;
    let notify_manager = Arc::new(NotifyManager::new());
    let extension_schemas = Arc::new(config.extension_schemas);
    let drain_timeout = config.drain_timeout;
    let idle_connection_timeout = config.idle_connection_timeout;
    let draining = Arc::new(AtomicBool::new(false));
    let (drain_tx, _) = watch::channel(false);

    // Session counters exposed as Prometheus gauges.
    let counters = SessionCounters::new();
    let metrics_ref = config.metrics.clone();
    let max_active_scans = config.max_active_scans;
    if let Some(ref metrics) = metrics_ref {
        metrics.set_resource_limits(
            config.max_active_scans as u64,
            config.stream_queue_depth as u64,
            config.max_buffered_rows as u64,
            config.max_response_bytes as u64,
        );
    }

    tokio::select! {
        result = async {
            loop {
                let (socket, addr) = listener.accept().await?;
                let permit = match session_semaphore.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        warn!(peer = %addr, "connection rejected: session capacity exhausted");
                        let _ = reject_connection(
                            socket,
                            "53300",
                            "too many connections",
                        )
                        .await;
                        continue;
                    }
                };
                if draining.load(Ordering::Acquire) {
                    let _ = reject_connection(socket, "57P01", "server is shutting down").await;
                    drop(permit);
                    continue;
                }

                let catalog = catalog.clone();
                let scans = scan_semaphore.clone();
                let tls = tls_acceptor.clone();
                let auth = auth_config.clone();
                let nm = notify_manager.clone();
                let es = extension_schemas.clone();
                let counters_ref = counters.clone();
                let metrics_task = metrics_ref.clone();
                let activity = ConnectionActivity::new(
                    counters_ref.clone(),
                    metrics_task.clone(),
                    draining.clone(),
                );
                counters_ref
                    .connections_open
                    .fetch_add(1, Ordering::AcqRel);
                counters_ref
                    .connections_idle
                    .fetch_add(1, Ordering::AcqRel);
                counters_ref.publish(metrics_task.as_ref());
                let drain_rx = drain_tx.subscribe();

                tokio::spawn(async move {
                    let _permit = permit;
                    let handlers = RockLakeServerHandlers::new_with_config_mode_and_limits_and_lifecycle(
                        catalog,
                        auth,
                        tls_required,
                        nm,
                        es,
                        access_mode,
                        scans,
                        max_active_scans,
                        config.max_buffered_rows,
                        config.max_response_bytes,
                        config.slow_operation_threshold,
                        metrics_task.clone(),
                        activity.clone(),
                    );
                    let connection_id = handlers.handler.connection_id();
                    let span = info_span!("pgwire_connection", connection_id = %connection_id);
                    info!(connection_id = %connection_id, peer = %addr, "New connection");

                    let process = pgwire::tokio::process_socket(socket, tls, handlers)
                        .instrument(span);
                    tokio::pin!(process);
                    tokio::select! {
                        result = &mut process => {
                            if let Err(e) = result {
                                error!(connection_id = %connection_id, peer = %addr, "Connection error: {e}");
                            }
                        }
                        reason = wait_for_connection_end(
                            activity.clone(),
                            idle_connection_timeout,
                            drain_rx,
                        ) => {
                            info!(
                                connection_id = %connection_id,
                                peer = %addr,
                                ?reason,
                                "connection closed by server policy"
                            );
                        }
                    }
                    debug!(connection_id = %connection_id, peer = %addr, "connection closed");

                    if activity.query_is_in_flight() {
                        activity.finish_query();
                    }
                    counters_ref
                        .connections_open
                        .fetch_sub(1, Ordering::AcqRel);
                    decrement_if_positive(&counters_ref.connections_idle);
                    counters_ref.publish(metrics_task.as_ref());
                });
            }
            #[allow(unreachable_code)]
            Ok::<(), std::io::Error>(())
        } => { result }
        _ = shutdown => {
            info!("Shutdown signal received; draining in-flight sessions (timeout: {:?})", drain_timeout);
            draining.store(true, Ordering::Release);
            drain_tx.send_replace(true);
            // Stop the listener and wait for active queries and sockets to close.
            drop(listener);
            let deadline = tokio::time::Instant::now() + drain_timeout;
            loop {
                if counters.connections_open.load(Ordering::Acquire) == 0 {
                    info!("All connections drained");
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    warn!("Drain timeout exceeded; forcing shutdown with {} active session(s)",
                        counters.connections_open.load(Ordering::Relaxed));
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connection_activity_tracks_query_and_idle_state() {
        let counters = SessionCounters::new();
        counters.connections_open.store(1, Ordering::Relaxed);
        counters.connections_idle.store(1, Ordering::Relaxed);
        let activity =
            ConnectionActivity::new(counters.clone(), None, Arc::new(AtomicBool::new(false)));

        let query = activity.begin_query();
        assert!(activity.query_is_in_flight());
        assert_eq!(counters.connections_idle.load(Ordering::Relaxed), 0);
        assert_eq!(counters.queries_in_flight.load(Ordering::Relaxed), 1);

        drop(query);
        assert!(!activity.query_is_in_flight());
        assert_eq!(counters.connections_idle.load(Ordering::Relaxed), 1);
        assert_eq!(counters.queries_in_flight.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn shutdown_waits_for_active_query_then_closes_connection() {
        let counters = SessionCounters::new();
        counters.connections_open.store(1, Ordering::Relaxed);
        counters.connections_idle.store(1, Ordering::Relaxed);
        let activity = ConnectionActivity::new(counters, None, Arc::new(AtomicBool::new(false)));
        let query = activity.begin_query();
        let (drain_tx, drain_rx) = watch::channel(false);
        let mut monitor = tokio::spawn(wait_for_connection_end(
            activity.clone(),
            Duration::from_secs(60),
            drain_rx,
        ));
        drain_tx.send(true).unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!monitor.is_finished());
        drop(query);

        let result = tokio::time::timeout(Duration::from_secs(1), &mut monitor)
            .await
            .expect("shutdown monitor should finish")
            .expect("shutdown monitor task should not panic");
        assert!(matches!(result, ConnectionEnd::Shutdown));
    }

    #[tokio::test]
    async fn idle_timeout_closes_idle_connection() {
        let activity = ConnectionActivity::standalone();
        let (_drain_tx, drain_rx) = watch::channel(false);
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            wait_for_connection_end(activity, Duration::from_millis(10), drain_rx),
        )
        .await
        .expect("idle timeout should finish");
        assert!(matches!(result, ConnectionEnd::IdleTimeout));
    }
}
