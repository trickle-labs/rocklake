//! Explicit ownership for PG-wire connection and request lifecycles.
//!
//! A connection owns session state and cancellation. A request owns admission,
//! timing, terminal state, and response observation. The protocol handlers only
//! translate protocol messages into these operations.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use pgwire::api::PgWireConnectionState;
use pgwire::error::{ErrorInfo, PgWireError};
use pgwire::messages::response::TransactionStatus;
use rocklake_catalog::metrics::CatalogMetrics;
use rocklake_router::CatalogId;
use tokio::sync::{watch, Mutex, Notify, OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::{info_span, warn, Span};

use crate::session::SessionState;

/// Stable connection counters shared by the server and connection contexts.
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

    pub(crate) fn publish(&self, metrics: Option<&Arc<CatalogMetrics>>) {
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

/// Shared connection activity used for idle timeout, drain, and query gauges.
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

    pub(crate) fn finish_query(&self) {
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

    pub(crate) fn query_is_in_flight(&self) -> bool {
        self.query_in_flight.load(Ordering::Acquire)
    }
}

pub(crate) fn decrement_if_positive(counter: &AtomicI64) -> bool {
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

/// Why the server stopped waiting for a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionEnd {
    IdleTimeout,
    Shutdown,
}

pub(crate) async fn wait_for_connection_end(
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

/// State recorded when a request stops running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestTerminalState {
    /// The protocol response completed successfully.
    Completed,
    /// The client cancelled the request.
    Cancelled,
    /// The client disconnected while work was active.
    ClientDisconnected,
    /// The request deadline expired.
    Timeout,
    /// The server stopped accepting or serving work.
    ServerShutdown,
    /// The protocol or response path failed.
    ProtocolError,
    /// Catalog or other request execution failed.
    Error,
}

/// Coarse operation class used by admission and tracing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationClass {
    Protocol,
    Interactive,
    InteractiveScan,
    Administrative,
    Copy,
}

/// The resource represented by an admission permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionKind {
    Connection,
    InteractiveScan,
    Administrative,
    ResponseBuffer,
}

/// A non-cloneable lease for one server resource.
pub struct AdmissionPermit {
    kind: AdmissionKind,
    _permit: OwnedSemaphorePermit,
    semaphore: Option<Arc<Semaphore>>,
    max: usize,
    metrics: Option<Arc<CatalogMetrics>>,
}

impl AdmissionPermit {
    pub(crate) fn connection(permit: OwnedSemaphorePermit) -> Self {
        Self {
            kind: AdmissionKind::Connection,
            _permit: permit,
            semaphore: None,
            max: 0,
            metrics: None,
        }
    }

    pub(crate) fn scan(
        permit: OwnedSemaphorePermit,
        semaphore: Arc<Semaphore>,
        max: usize,
        metrics: Option<Arc<CatalogMetrics>>,
    ) -> Self {
        Self {
            kind: AdmissionKind::InteractiveScan,
            _permit: permit,
            semaphore: Some(semaphore),
            max,
            metrics,
        }
    }

    pub fn administrative(permit: OwnedSemaphorePermit) -> Self {
        Self {
            kind: AdmissionKind::Administrative,
            _permit: permit,
            semaphore: None,
            max: 0,
            metrics: None,
        }
    }

    pub(crate) fn response_buffer(permit: OwnedSemaphorePermit) -> Self {
        Self {
            kind: AdmissionKind::ResponseBuffer,
            _permit: permit,
            semaphore: None,
            max: 0,
            metrics: None,
        }
    }

    /// Return the resource held by this lease.
    pub fn kind(&self) -> AdmissionKind {
        self.kind
    }
}

impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        if self.kind == AdmissionKind::InteractiveScan {
            if let (Some(metrics), Some(semaphore)) = (&self.metrics, &self.semaphore) {
                metrics.set_active_scans(
                    self.max
                        .saturating_sub(semaphore.available_permits().saturating_add(1))
                        as u64,
                );
            }
        }
    }
}

/// State owned by one accepted connection.
pub struct ConnectionContext {
    connection_id: uuid::Uuid,
    session: Arc<Mutex<SessionState>>,
    principal: StdMutex<Option<String>>,
    catalog_route: StdMutex<Option<String>>,
    catalog_id: StdMutex<Option<CatalogId>>,
    protocol_state: StdMutex<PgWireConnectionState>,
    transaction_status: StdMutex<TransactionStatus>,
    cancellation: CancellationToken,
    activity: Arc<ConnectionActivity>,
}

impl ConnectionContext {
    pub(crate) fn new(
        counters: Arc<SessionCounters>,
        metrics: Option<Arc<CatalogMetrics>>,
        draining: Arc<AtomicBool>,
    ) -> Arc<Self> {
        Arc::new(Self {
            connection_id: crate::telemetry::request_id(),
            session: Arc::new(Mutex::new(SessionState::new())),
            principal: StdMutex::new(None),
            catalog_route: StdMutex::new(None),
            catalog_id: StdMutex::new(None),
            protocol_state: StdMutex::new(PgWireConnectionState::AwaitingSslRequest),
            transaction_status: StdMutex::new(TransactionStatus::Idle),
            cancellation: CancellationToken::new(),
            activity: ConnectionActivity::new(counters, metrics, draining),
        })
    }

    pub(crate) fn standalone(metrics: Option<Arc<CatalogMetrics>>) -> Arc<Self> {
        Self::new(
            SessionCounters::new(),
            metrics,
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// Stable identity for logs, traces, and client-visible correlation data.
    pub fn connection_id(&self) -> uuid::Uuid {
        self.connection_id
    }

    /// Session data shared by the simple, extended, and COPY handlers.
    pub fn session(&self) -> Arc<Mutex<SessionState>> {
        self.session.clone()
    }

    pub(crate) fn activity(&self) -> &Arc<ConnectionActivity> {
        &self.activity
    }

    pub(crate) fn is_draining(&self) -> bool {
        self.activity.is_draining()
    }

    pub(crate) fn touch(&self) {
        self.activity.touch();
    }

    /// Cancel all request work owned by this connection.
    pub fn cancel(&self) {
        self.cancellation.cancel();
        self.activity.touch();
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn set_principal(&self, principal: impl Into<String>) {
        *self.principal.lock().expect("principal mutex poisoned") = Some(principal.into());
    }

    pub fn principal(&self) -> Option<String> {
        self.principal
            .lock()
            .expect("principal mutex poisoned")
            .clone()
    }

    pub fn select_catalog_route(&self, route: impl Into<String>) {
        *self
            .catalog_route
            .lock()
            .expect("catalog route mutex poisoned") = Some(route.into());
    }

    pub fn catalog_route(&self) -> Option<String> {
        self.catalog_route
            .lock()
            .expect("catalog route mutex poisoned")
            .clone()
    }

    /// Bind this connection to a stable catalog identity after route resolution.
    pub fn bind_catalog_id(&self, id: CatalogId) {
        let mut bound = self.catalog_id.lock().expect("catalog id mutex poisoned");
        if bound.is_none() {
            *bound = Some(id);
        }
    }

    /// Return the stable catalog identity bound to this connection.
    pub fn catalog_id(&self) -> Option<CatalogId> {
        self.catalog_id
            .lock()
            .expect("catalog id mutex poisoned")
            .clone()
    }

    pub(crate) fn set_protocol_state(&self, state: PgWireConnectionState) {
        *self
            .protocol_state
            .lock()
            .expect("protocol state mutex poisoned") = state;
    }

    pub fn protocol_state(&self) -> PgWireConnectionState {
        *self
            .protocol_state
            .lock()
            .expect("protocol state mutex poisoned")
    }

    pub(crate) fn set_transaction_status(&self, status: TransactionStatus) {
        *self
            .transaction_status
            .lock()
            .expect("transaction status mutex poisoned") = status;
    }

    pub fn transaction_status(&self) -> TransactionStatus {
        *self
            .transaction_status
            .lock()
            .expect("transaction status mutex poisoned")
    }

    /// Start one request owned by this connection.
    pub fn begin_request(
        self: &Arc<Self>,
        operation: OperationClass,
        slow_operation_threshold: Duration,
    ) -> RequestContext {
        let query_id = crate::telemetry::request_id();
        RequestContext {
            state: Arc::new(RequestState {
                connection: self.clone(),
                query_id,
                started: Instant::now(),
                deadline: StdMutex::new(None),
                cancellation: self.cancellation.child_token(),
                operation: StdMutex::new(operation),
                selected_snapshot: StdMutex::new(None),
                span: info_span!(
                    "pgwire_request",
                    connection_id = %self.connection_id,
                    query_id = %query_id,
                    operation = ?operation,
                ),
                finished: AtomicBool::new(false),
                query_guard: StdMutex::new(Some(self.activity.begin_query())),
                slow_operation_threshold,
                metrics: self.activity.metrics.clone(),
                skip_next_response: AtomicBool::new(false),
            }),
        }
    }
}

struct RequestState {
    connection: Arc<ConnectionContext>,
    query_id: uuid::Uuid,
    started: Instant,
    deadline: StdMutex<Option<Instant>>,
    cancellation: CancellationToken,
    operation: StdMutex<OperationClass>,
    selected_snapshot: StdMutex<Option<u64>>,
    span: Span,
    finished: AtomicBool,
    query_guard: StdMutex<Option<QueryGuard>>,
    slow_operation_threshold: Duration,
    metrics: Option<Arc<CatalogMetrics>>,
    skip_next_response: AtomicBool,
}

impl RequestState {
    fn finish(&self, terminal: RequestTerminalState) {
        if self.finished.swap(true, Ordering::AcqRel) {
            return;
        }
        let elapsed = self.started.elapsed();
        if let Some(metrics) = &self.metrics {
            metrics.record_pgwire_query(elapsed.as_micros() as u64);
        }
        if terminal != RequestTerminalState::Completed {
            warn!(
                query_id = %self.query_id,
                connection_id = %self.connection.connection_id,
                terminal = ?terminal,
                operation = ?*self.operation.lock().expect("request operation mutex poisoned"),
                elapsed_ms = elapsed.as_millis() as u64,
                "pgwire request terminated"
            );
        } else if elapsed >= self.slow_operation_threshold {
            warn!(
                query_id = %self.query_id,
                connection_id = %self.connection.connection_id,
                operation = ?*self.operation.lock().expect("request operation mutex poisoned"),
                elapsed_ms = elapsed.as_millis() as u64,
                "slow operation"
            );
        }
        self.query_guard
            .lock()
            .expect("request guard mutex poisoned")
            .take();
    }
}

impl Drop for RequestState {
    fn drop(&mut self) {
        self.finish(RequestTerminalState::ClientDisconnected);
    }
}

/// Request-scoped identity, cancellation, admission timing, and completion.
#[derive(Clone)]
pub struct RequestContext {
    state: Arc<RequestState>,
}

impl RequestContext {
    pub fn query_id(&self) -> uuid::Uuid {
        self.state.query_id
    }

    pub fn connection_id(&self) -> uuid::Uuid {
        self.state.connection.connection_id
    }

    pub fn operation_class(&self) -> OperationClass {
        *self
            .state
            .operation
            .lock()
            .expect("request operation mutex poisoned")
    }

    pub fn set_operation_class(&self, operation: OperationClass) {
        *self
            .state
            .operation
            .lock()
            .expect("request operation mutex poisoned") = operation;
    }

    pub fn set_deadline(&self, deadline: Option<Instant>) {
        *self
            .state
            .deadline
            .lock()
            .expect("request deadline mutex poisoned") = deadline;
    }

    pub fn deadline(&self) -> Option<Instant> {
        *self
            .state
            .deadline
            .lock()
            .expect("request deadline mutex poisoned")
    }

    pub fn set_selected_snapshot(&self, snapshot: Option<u64>) {
        *self
            .state
            .selected_snapshot
            .lock()
            .expect("request snapshot mutex poisoned") = snapshot;
    }

    pub fn selected_snapshot(&self) -> Option<u64> {
        *self
            .state
            .selected_snapshot
            .lock()
            .expect("request snapshot mutex poisoned")
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.state.cancellation.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.cancellation.is_cancelled()
    }

    pub async fn cancelled(&self) {
        self.state.cancellation.cancelled().await;
    }

    pub fn cancellation_error(&self) -> PgWireError {
        PgWireError::UserError(Box::new(ErrorInfo::new(
            "ERROR".to_string(),
            "57014".to_string(),
            "canceling statement due to user request".to_string(),
        )))
    }

    pub fn span(&self) -> Span {
        self.state.span.clone()
    }

    pub(crate) fn metrics(&self) -> Option<Arc<CatalogMetrics>> {
        self.state.metrics.clone()
    }

    pub(crate) fn record_admission(&self, started: Instant) {
        if let Some(metrics) = &self.state.metrics {
            metrics.observe_pgwire_admission_us(started.elapsed().as_micros() as u64);
        }
    }

    pub(crate) fn record_classification(&self, started: Instant) {
        if let Some(metrics) = &self.state.metrics {
            metrics.observe_sql_classification_us(started.elapsed().as_micros() as u64);
        }
    }

    pub(crate) fn record_execution(&self, started: Instant) {
        if let Some(metrics) = &self.state.metrics {
            metrics.observe_pgwire_execution_us(started.elapsed().as_micros() as u64);
        }
    }

    pub(crate) fn record_error(&self, error: &PgWireError) {
        let sqlstate = match error {
            PgWireError::UserError(info) => info.code.as_str(),
            _ => "XX000",
        };
        if let Some(metrics) = &self.state.metrics {
            metrics.record_pgwire_error(sqlstate);
        }
        tracing::error!(
            query_id = %self.query_id(),
            connection_id = %self.connection_id(),
            sqlstate,
            error = %error,
            "query failed"
        );
    }

    pub(crate) fn record_error_info(&self, info: &ErrorInfo) {
        if let Some(metrics) = &self.state.metrics {
            metrics.record_pgwire_error(&info.code);
        }
        tracing::error!(
            query_id = %self.query_id(),
            connection_id = %self.connection_id(),
            sqlstate = %info.code,
            error = %info.message,
            "query failed"
        );
    }

    pub(crate) fn skip_next_response_observation(&self) {
        self.state.skip_next_response.store(true, Ordering::Release);
    }

    pub(crate) fn take_skipped_response_observation(&self) -> bool {
        self.state.skip_next_response.swap(false, Ordering::AcqRel)
    }

    pub fn finish(&self, terminal: RequestTerminalState) {
        self.state.finish(terminal);
    }

    pub fn response_observer(&self) -> ResponseObserver {
        ResponseObserver {
            request: self.clone(),
            started: Instant::now(),
            first_row: None,
            rows: 0,
            bytes: 0,
            terminal: RequestTerminalState::ProtocolError,
            finished: false,
        }
    }
}

/// Exactly-once observation of one protocol response.
pub struct ResponseObserver {
    pub(crate) request: RequestContext,
    started: Instant,
    first_row: Option<Instant>,
    rows: u64,
    bytes: u64,
    terminal: RequestTerminalState,
    finished: bool,
}

impl ResponseObserver {
    pub fn observe_row(&mut self, bytes: usize) {
        self.rows = self.rows.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes as u64);
    }

    pub fn mark_first_row(&mut self) {
        self.first_row.get_or_insert_with(Instant::now);
    }

    pub fn set_terminal(&mut self, terminal: RequestTerminalState) {
        self.terminal = terminal;
    }

    pub fn finish(mut self) {
        self.terminal = RequestTerminalState::Completed;
        self.complete();
    }

    fn complete(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(metrics) = self.request.metrics() {
            let response_elapsed = self.started.elapsed();
            let ttfr = self
                .first_row
                .map(|first| first.duration_since(self.request.state.started).as_micros() as u64)
                .unwrap_or(0);
            metrics.record_pgwire_response_with_timing(
                self.rows,
                self.bytes,
                ttfr,
                response_elapsed.as_micros() as u64,
            );
            metrics.observe_pgwire_response_delivery_us(response_elapsed.as_micros() as u64);
        }
    }
}

impl Drop for ResponseObserver {
    fn drop(&mut self) {
        self.complete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn request_and_response_complete_once() {
        let metrics = Arc::new(CatalogMetrics::new(2));
        let connection = ConnectionContext::standalone(Some(metrics.clone()));
        let request = connection.begin_request(OperationClass::Interactive, Duration::from_secs(1));
        let mut response = request.response_observer();
        response.observe_row(12);
        response.mark_first_row();
        response.finish();
        assert_eq!(metrics.pgwire_queries_total.load(Ordering::Relaxed), 0);
        request.finish(RequestTerminalState::Completed);

        assert_eq!(metrics.pgwire_queries_total.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.pgwire_response_rows.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.pgwire_response_bytes.load(Ordering::Relaxed), 12);
    }

    #[tokio::test]
    async fn cancellation_propagates_to_request() {
        let connection = ConnectionContext::standalone(None);
        let request = connection.begin_request(OperationClass::Interactive, Duration::from_secs(1));
        connection.cancel();
        tokio::time::timeout(Duration::from_secs(1), request.cancelled())
            .await
            .expect("request cancellation should fire");
        assert!(request.is_cancelled());
    }

    #[test]
    fn admission_permits_are_typed_and_non_cloneable() {
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = semaphore.clone().try_acquire_owned().unwrap();
        let permit = AdmissionPermit::response_buffer(permit);
        assert_eq!(permit.kind(), AdmissionKind::ResponseBuffer);
    }
}
