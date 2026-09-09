//! PG Wire protocol handler implementation.
//!
//! Implements SimpleQueryHandler and ExtendedQueryHandler for the pgwire crate.
//! Supports optional password authentication (cleartext with constant-time comparison).

use std::fmt::Debug;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};
use futures::sink::{Sink, SinkExt};
use futures::StreamExt;
use pgwire::api::auth::{
    finish_authentication, save_startup_parameters_to_metadata, DefaultServerParameterProvider,
};
use pgwire::api::copy::CopyHandler;
use pgwire::api::portal::Portal;
use pgwire::api::query::{
    send_execution_response, send_ready_for_query, ExtendedQueryHandler, SimpleQueryHandler,
};
use pgwire::api::results::FieldInfo;
use pgwire::api::results::{
    DescribePortalResponse, DescribeStatementResponse, QueryResponse, Response,
};
use pgwire::api::stmt::{QueryParser, StoredStatement};
use pgwire::api::store::PortalStore;
use pgwire::api::{
    ClientInfo, ClientPortalStore, NoopErrorHandler, PgWireConnectionState, PgWireServerHandlers,
    Type, METADATA_DATABASE, METADATA_USER,
};
use pgwire::error::{ErrorInfo, PgWireError, PgWireResult};
use pgwire::messages::data::RowDescription;
use pgwire::messages::response::{CommandComplete, EmptyQueryResponse, ErrorResponse};
use pgwire::messages::startup::Authentication;
use pgwire::messages::{copy::CopyOutResponse, PgWireBackendMessage, PgWireFrontendMessage};
use sqlparser::ast::{Expr, SelectItem, SetExpr, Statement};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use tokio::sync::{Mutex, Semaphore};
use tracing::Instrument;

use rocklake_catalog::metrics::CatalogMetrics;
use rocklake_catalog::CatalogStore;
use rocklake_core::rows::ColumnRow;
use rocklake_sql::{classify_statement, ParamValues, StatementKind};

use crate::copy_parser;
use crate::executor;
use crate::lifecycle::{
    AdmissionPermit, ConnectionContext, OperationClass, RequestContext, RequestTerminalState,
};
use crate::notify::NotifyManager;
use crate::server::{server_shutting_down_error, AuthConfig};
use crate::session::{BootstrapSchemaRow, SessionState};

/// RockLake COPY handler: parses binary COPY FROM STDIN data for ducklake_*
/// tables and stores the bootstrap rows in the session for later commit.
#[derive(Clone)]
pub struct RockLakeCopyHandler {
    session: Arc<Mutex<SessionState>>,
    connection: Arc<ConnectionContext>,
    copy_request: Arc<StdMutex<Option<RequestContext>>>,
    mode: executor::AccessMode,
}

impl RockLakeCopyHandler {
    pub fn new(session: Arc<Mutex<SessionState>>) -> Self {
        Self::new_with_mode(session, executor::AccessMode::Writer)
    }

    pub fn new_with_mode(session: Arc<Mutex<SessionState>>, mode: executor::AccessMode) -> Self {
        Self {
            session,
            connection: ConnectionContext::standalone(None),
            copy_request: Arc::new(StdMutex::new(None)),
            mode,
        }
    }

    fn new_with_connection(connection: Arc<ConnectionContext>, mode: executor::AccessMode) -> Self {
        Self {
            session: connection.session(),
            connection,
            copy_request: Arc::new(StdMutex::new(None)),
            mode,
        }
    }

    fn begin_copy_request(&self) -> RequestContext {
        let mut request = self
            .copy_request
            .lock()
            .expect("copy request mutex poisoned");
        request
            .get_or_insert_with(|| {
                self.connection
                    .begin_request(OperationClass::Copy, Duration::from_secs(1))
            })
            .clone()
    }

    fn take_copy_request(&self) -> RequestContext {
        let request = self
            .copy_request
            .lock()
            .expect("copy request mutex poisoned")
            .take();
        request.unwrap_or_else(|| self.begin_copy_request())
    }
}

#[async_trait]
impl CopyHandler for RockLakeCopyHandler {
    async fn on_copy_data<C>(
        &self,
        _client: &mut C,
        copy_data: pgwire::messages::copy::CopyData,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let query = self.begin_copy_request();
        self.connection.touch();
        if self.mode == executor::AccessMode::Reader {
            let error = read_only_error();
            query.record_error(&error);
            query.finish(RequestTerminalState::Error);
            return Err(error);
        }
        // Append incoming bytes to the accumulator for the active COPY table.
        let mut session = tokio::select! {
            session = self.session.lock() => session,
            _ = query.cancelled() => {
                let error = query.cancellation_error();
                query.finish(RequestTerminalState::Cancelled);
                return Err(error);
            }
        };
        if let Some(acc) = &mut session.pending_copy {
            acc.data.extend_from_slice(&copy_data.data);
        }
        Ok(())
    }

    async fn on_copy_done<C>(
        &self,
        client: &mut C,
        _done: pgwire::messages::copy::CopyDone,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let query = self.take_copy_request();
        let mut observation = query.response_observer();
        self.connection.touch();
        if self.mode == executor::AccessMode::Reader {
            let error = read_only_error();
            observation.set_terminal(RequestTerminalState::Error);
            query.record_error(&error);
            query.finish(RequestTerminalState::Error);
            return Err(error);
        }
        // Drain the accumulator outside the lock to avoid holding it during parsing.
        let (table, data) = {
            let mut session = self.session.lock().await;
            match session.pending_copy.take() {
                Some(acc) => (acc.table, acc.data),
                None => {
                    // No active COPY — still send CommandComplete.
                    let result = send_copy_done(client, 0).await;
                    match &result {
                        Ok(()) => {
                            observation.finish();
                            query.finish(RequestTerminalState::Completed);
                        }
                        Err(error) => {
                            observation.set_terminal(RequestTerminalState::ProtocolError);
                            query.record_error(error);
                            query.finish(RequestTerminalState::ProtocolError);
                        }
                    }
                    return result;
                }
            }
        };

        // Parse the binary COPY stream and extract bootstrap rows.
        let rows = match copy_parser::parse_binary_copy_rows(&data) {
            Ok(rows) => rows,
            Err(e) => {
                // Binary COPY stream is malformed — fail closed with SQLSTATE 08P01
                // (protocol violation). Do NOT mark bootstrap as complete.
                let msg = format!("binary COPY parse error for table '{table}': {e}");
                client
                    .send(PgWireBackendMessage::ErrorResponse(ErrorResponse::from(
                        ErrorInfo::new("ERROR".to_string(), "08P01".to_string(), msg.clone()),
                    )))
                    .await
                    .ok();
                observation.set_terminal(RequestTerminalState::ProtocolError);
                let error = PgWireError::UserError(Box::new(ErrorInfo::new(
                    "ERROR".to_string(),
                    "08P01".to_string(),
                    msg,
                )));
                query.record_error(&error);
                query.finish(RequestTerminalState::ProtocolError);
                return Err(error);
            }
        };
        let row_count = rows.len();

        {
            let mut session = self.session.lock().await;
            match table.as_str() {
                "ducklake_snapshot" if !rows.is_empty() => {
                    // Any row means DuckDB has initialised a snapshot.
                    session.bootstrap.has_snapshot = true;
                }
                "ducklake_schema" => {
                    for row in &rows {
                        // ducklake_schema column order:
                        //   0: schema_id (BIGINT)
                        //   1: schema_uuid (UUID)
                        //   2: begin_snapshot (BIGINT)
                        //   3: end_snapshot (BIGINT, nullable)
                        //   4: schema_name (VARCHAR)  ← what we need
                        //   5: path (VARCHAR, nullable)
                        //   6: path_is_relative (BOOLEAN, nullable)
                        if let Some(name) = copy_parser::extract_varchar(row, 4) {
                            session
                                .bootstrap
                                .schemas
                                .push(BootstrapSchemaRow { schema_name: name });
                        }
                    }
                }
                // ducklake_snapshot_changes and ducklake_metadata are accepted
                // but not persisted; they don't affect catalog bootstrap state.
                _ => {}
            }
        }

        let result = send_copy_done(client, row_count).await;
        match &result {
            Ok(()) => {
                observation.finish();
                query.finish(RequestTerminalState::Completed);
            }
            Err(error) => {
                observation.set_terminal(RequestTerminalState::ProtocolError);
                query.record_error(error);
                query.finish(RequestTerminalState::ProtocolError);
            }
        }
        result
    }

    async fn on_copy_fail<C>(
        &self,
        _client: &mut C,
        fail: pgwire::messages::copy::CopyFail,
    ) -> PgWireError
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let error = PgWireError::UserError(Box::new(ErrorInfo::new(
            "ERROR".to_owned(),
            "57014".to_owned(),
            format!("COPY IN mode terminated by the user: {}", fail.message),
        )));
        let query = self
            .copy_request
            .lock()
            .expect("copy request mutex poisoned")
            .take();
        if let Some(query) = query {
            query.record_error(&error);
            query.finish(RequestTerminalState::Cancelled);
        }
        error
    }
}

fn read_only_error() -> PgWireError {
    PgWireError::UserError(Box::new(ErrorInfo::new(
        "ERROR".to_string(),
        "25006".to_string(),
        "cannot execute write operation in a read-only transaction".to_string(),
    )))
}

/// Send `CommandComplete "COPY N"` to the client.
async fn send_copy_done<C>(client: &mut C, rows: usize) -> PgWireResult<()>
where
    C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
    C::Error: Debug,
    PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
{
    use pgwire::messages::response::CommandComplete;
    client
        .send(PgWireBackendMessage::CommandComplete(CommandComplete::new(
            format!("COPY {rows}"),
        )))
        .await?;
    Ok(())
}

/// The main RockLake query handler.
pub struct RockLakeHandler {
    pub catalog: Arc<Mutex<CatalogStore>>,
    pub connection: Arc<ConnectionContext>,
    pub parser: Arc<RockLakeQueryParser>,
    pub auth: Arc<AuthConfig>,
    /// Shared LISTEN/NOTIFY manager for this server instance.
    pub notify_manager: Arc<NotifyManager>,
    /// Allowed extension schema names (configurable via --extension-schemas).
    pub extension_schemas: Arc<Vec<String>>,
    pub access_mode: executor::AccessMode,
    scan_semaphore: Arc<Semaphore>,
    max_active_scans: usize,
    max_buffered_rows: usize,
    max_response_bytes: usize,
    response_buffer: Arc<Semaphore>,
    slow_operation_threshold: Duration,
}

fn wrap_query_response<'a>(
    response: Response<'a>,
    permit: Option<AdmissionPermit>,
    max_buffered_rows: usize,
    max_response_bytes: usize,
    response_buffer: Arc<Semaphore>,
    query: RequestContext,
) -> Response<'a> {
    let Response::Query(query_response) = response else {
        return response;
    };

    let schema = query_response.row_schema();
    let command_tag = query_response.command_tag().to_string();
    let rows = query_response.data_rows();
    let state = (rows, permit, 0usize, query.response_observer());
    let rows = futures::stream::unfold(state, move |(mut rows, permit, bytes, mut observation)| {
        let response_buffer = response_buffer.clone();
        async move {
            if observation.request.is_cancelled() {
                observation.set_terminal(RequestTerminalState::Cancelled);
                return Some((
                    Err(observation.request.cancellation_error()),
                    (rows, permit, bytes, observation),
                ));
            }
            let Some(item) = rows.next().await else {
                observation.set_terminal(RequestTerminalState::Completed);
                let request = observation.request.clone();
                observation.finish();
                request.finish(RequestTerminalState::Completed);
                return None;
            };
            let mut next_bytes = bytes;
            let item = match item {
                Ok(row) => {
                    if max_buffered_rows < 1 {
                        if let Some(metrics) = observation.request.metrics() {
                            metrics.increment_resource_limit_exhaustions();
                        }
                        observation.set_terminal(RequestTerminalState::Error);
                        return Some((
                            Err(resource_limit_error("buffered row limit exhausted")),
                            (rows, permit, bytes, observation),
                        ));
                    }
                    let buffer_permit = match response_buffer.clone().try_acquire_owned() {
                        Ok(permit) => AdmissionPermit::response_buffer(permit),
                        Err(_) => {
                            if let Some(metrics) = observation.request.metrics() {
                                metrics.increment_resource_limit_exhaustions();
                            }
                            observation.set_terminal(RequestTerminalState::Error);
                            return Some((
                                Err(resource_limit_error("response buffer capacity exhausted")),
                                (rows, permit, bytes, observation),
                            ));
                        }
                    };
                    next_bytes = bytes.saturating_add(row.data.len());
                    if next_bytes > max_response_bytes {
                        if let Some(metrics) = observation.request.metrics() {
                            metrics.increment_resource_limit_exhaustions();
                        }
                        observation.set_terminal(RequestTerminalState::Error);
                        Err(resource_limit_error("response byte limit exhausted"))
                    } else {
                        observation.observe_row(row.data.len());
                        observation.mark_first_row();
                        drop(buffer_permit);
                        Ok(row)
                    }
                }
                Err(error) => {
                    observation.set_terminal(RequestTerminalState::Error);
                    Err(error)
                }
            };
            Some((item, (rows, permit, next_bytes, observation)))
        }
    })
    .boxed();
    let mut wrapped = QueryResponse::new(schema, rows);
    wrapped.set_command_tag(&command_tag);
    Response::Query(wrapped)
}

fn resource_limit_error(message: &str) -> PgWireError {
    PgWireError::UserError(Box::new(ErrorInfo::new(
        "ERROR".to_string(),
        "54001".to_string(),
        message.to_string(),
    )))
}

impl RockLakeHandler {
    pub fn new(catalog: Arc<Mutex<CatalogStore>>) -> Self {
        Self::new_with_mode(catalog, executor::AccessMode::Writer)
    }

    pub fn new_with_mode(
        catalog: Arc<Mutex<CatalogStore>>,
        access_mode: executor::AccessMode,
    ) -> Self {
        let connection = ConnectionContext::standalone(None);
        Self {
            catalog,
            connection,
            parser: Arc::new(RockLakeQueryParser),
            auth: Arc::new(AuthConfig::default()),
            notify_manager: Arc::new(NotifyManager::new()),
            extension_schemas: Arc::new(vec!["pgtrickle".to_string()]),
            access_mode,
            scan_semaphore: Arc::new(Semaphore::new(25)),
            max_active_scans: 25,
            max_buffered_rows: 1024,
            max_response_bytes: usize::MAX,
            response_buffer: Arc::new(Semaphore::new(1024)),
            slow_operation_threshold: Duration::from_secs(1),
        }
    }

    pub fn new_with_auth(catalog: Arc<Mutex<CatalogStore>>, auth: Arc<AuthConfig>) -> Self {
        Self::new_with_auth_mode(catalog, auth, executor::AccessMode::Writer)
    }

    pub fn new_with_auth_mode(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        access_mode: executor::AccessMode,
    ) -> Self {
        let connection = ConnectionContext::standalone(None);
        Self {
            catalog,
            connection,
            parser: Arc::new(RockLakeQueryParser),
            auth,
            notify_manager: Arc::new(NotifyManager::new()),
            extension_schemas: Arc::new(vec!["pgtrickle".to_string()]),
            access_mode,
            scan_semaphore: Arc::new(Semaphore::new(25)),
            max_active_scans: 25,
            max_buffered_rows: 1024,
            max_response_bytes: usize::MAX,
            response_buffer: Arc::new(Semaphore::new(1024)),
            slow_operation_threshold: Duration::from_secs(1),
        }
    }

    pub fn new_with_config(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
    ) -> Self {
        Self::new_with_config_mode(
            catalog,
            auth,
            notify_manager,
            extension_schemas,
            executor::AccessMode::Writer,
        )
    }

    pub fn new_with_config_mode(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
        access_mode: executor::AccessMode,
    ) -> Self {
        Self::new_with_config_mode_and_limits(
            catalog,
            auth,
            notify_manager,
            extension_schemas,
            access_mode,
            Arc::new(Semaphore::new(25)),
            25,
            1024,
            usize::MAX,
            Duration::from_secs(1),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_config_mode_and_limits(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
        access_mode: executor::AccessMode,
        scan_semaphore: Arc<Semaphore>,
        max_active_scans: usize,
        max_buffered_rows: usize,
        max_response_bytes: usize,
        slow_operation_threshold: Duration,
        metrics: Option<Arc<CatalogMetrics>>,
    ) -> Self {
        Self::new_with_config_mode_and_limits_and_lifecycle(
            catalog,
            auth,
            notify_manager,
            extension_schemas,
            access_mode,
            scan_semaphore,
            max_active_scans,
            max_buffered_rows,
            max_response_bytes,
            slow_operation_threshold,
            ConnectionContext::standalone(metrics),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_config_mode_and_limits_and_lifecycle(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
        access_mode: executor::AccessMode,
        scan_semaphore: Arc<Semaphore>,
        max_active_scans: usize,
        max_buffered_rows: usize,
        max_response_bytes: usize,
        slow_operation_threshold: Duration,
        connection: Arc<ConnectionContext>,
    ) -> Self {
        Self {
            catalog,
            connection,
            parser: Arc::new(RockLakeQueryParser),
            auth,
            notify_manager,
            extension_schemas,
            access_mode,
            scan_semaphore,
            max_active_scans,
            max_buffered_rows,
            max_response_bytes,
            response_buffer: Arc::new(Semaphore::new(max_buffered_rows.max(1))),
            slow_operation_threshold,
        }
    }

    fn request_context(&self, operation: OperationClass) -> RequestContext {
        self.connection
            .begin_request(operation, self.slow_operation_threshold)
    }

    pub(crate) fn connection_id(&self) -> uuid::Uuid {
        self.connection.connection_id()
    }

    fn classify_sql(&self, sql: &str, query: &RequestContext) -> StatementKind {
        let classification_started = Instant::now();
        let kind = classify_statement(sql).unwrap_or(StatementKind::Unsupported(String::new()));
        query.record_classification(classification_started);
        query.set_operation_class(match kind {
            StatementKind::CopyToStdout { .. } => OperationClass::Copy,
            StatementKind::SelectDataFiles
            | StatementKind::SelectDataFilesWithLimit
            | StatementKind::SelectFileColumnStats
            | StatementKind::SelectDeleteFiles => OperationClass::InteractiveScan,
            _ => OperationClass::Interactive,
        });
        tracing::debug!(
            query_id = %query.query_id(),
            connection_id = %query.connection_id(),
            statement_kind = ?kind,
            "classified SQL statement"
        );
        kind
    }

    async fn acquire_scan(
        &self,
        kind: &StatementKind,
        query: &RequestContext,
    ) -> PgWireResult<Option<AdmissionPermit>> {
        let admission_started = Instant::now();
        if !matches!(
            kind,
            StatementKind::SelectDataFiles
                | StatementKind::SelectDataFilesWithLimit
                | StatementKind::SelectFileColumnStats
                | StatementKind::SelectDeleteFiles
        ) {
            query.record_admission(admission_started);
            return Ok(None);
        }
        // Waiting is cancellation-safe: dropping the acquire future releases
        // the queue slot without consuming a permit.
        let permit = tokio::select! {
            permit = self.scan_semaphore.clone().acquire_owned() => permit.map_err(|_| {
                if let Some(metrics) = query.metrics() {
                    metrics.increment_resource_limit_exhaustions();
                }
                resource_limit_error("active catalog scan limit unavailable")
            }),
            _ = query.cancelled() => Err(query.cancellation_error()),
        };
        query.record_admission(admission_started);
        let permit = permit?;
        if let Some(metrics) = query.metrics() {
            metrics.set_active_scans(
                self.max_active_scans
                    .saturating_sub(self.scan_semaphore.available_permits()) as u64,
            );
        }
        Ok(Some(AdmissionPermit::scan(
            permit,
            self.scan_semaphore.clone(),
            self.max_active_scans,
            query.metrics(),
        )))
    }

    /// If `sql` is a `COPY (SELECT ...) TO STDOUT`, execute the inner SELECT
    /// and stream PostgreSQL binary COPY frames directly to the client.
    async fn try_stream_copy_to_stdout<C>(
        &self,
        client: &mut C,
        kind: &StatementKind,
        query: &RequestContext,
    ) -> PgWireResult<Option<usize>>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let StatementKind::CopyToStdout { query: inner_sql } = kind else {
            return Ok(None);
        };

        let inner_kind = self.classify_sql(inner_sql, query);
        let _scan_permit = self.acquire_scan(&inner_kind, query).await?;

        let params = ParamValues::default();
        let execution_started = Instant::now();
        let session_handle = self.connection.session();
        let mut session = session_handle.lock().await;
        let result = tokio::select! {
            result = executor::execute_sql_with_mode(
                inner_sql,
                &params,
                &self.catalog,
                &mut session,
                &self.notify_manager,
                &self.extension_schemas,
                self.access_mode,
            ) => result,
            _ = query.cancelled() => {
                query.record_execution(execution_started);
                return Err(query.cancellation_error());
            }
        };
        query.record_execution(execution_started);
        let mut responses = result.map_err(|e| -> PgWireError { e.into() })?;

        let response = responses.pop().ok_or_else(|| {
            PgWireError::UserError(Box::new(ErrorInfo::new(
                "ERROR".to_string(),
                "XX000".to_string(),
                "COPY TO STDOUT inner query returned no response".to_string(),
            )))
        })?;

        let pgwire::api::results::Response::Query(query_response) = response else {
            return Err(PgWireError::UserError(Box::new(ErrorInfo::new(
                "ERROR".to_string(),
                "XX000".to_string(),
                "COPY TO STDOUT inner statement must return rows".to_string(),
            ))));
        };

        let row_schema = query_response.row_schema();
        let projected_indices = projected_copy_indices(inner_sql, row_schema.as_ref());
        let columns = projected_indices.len();
        let mut observation = query.response_observer();
        client
            .send(PgWireBackendMessage::CopyOutResponse(CopyOutResponse::new(
                1,
                columns as i16,
                vec![1; columns],
            )))
            .await?;

        let mut row_count = 0usize;
        let mut response_bytes = 0usize;
        let mut payload = BytesMut::from(binary_copy_header().as_ref());
        let mut rows = query_response.data_rows();
        while let Some(row_result) = rows.next().await {
            if query.is_cancelled() {
                observation.set_terminal(RequestTerminalState::Cancelled);
                return Err(query.cancellation_error());
            }
            let row = match row_result {
                Ok(row) => row,
                Err(error) => {
                    observation.set_terminal(RequestTerminalState::Error);
                    return Err(error);
                }
            };
            let projected_row = match project_copy_row_data(
                &row.data,
                row.field_count as usize,
                &projected_indices,
                row_schema.as_ref(),
            ) {
                Ok(projected_row) => projected_row,
                Err(error) => {
                    observation.set_terminal(RequestTerminalState::Error);
                    return Err(error);
                }
            };
            let row_bytes = 2usize
                .saturating_add(projected_row.len())
                .saturating_add(columns.saturating_mul(4));
            response_bytes = response_bytes.saturating_add(row_bytes);
            if response_bytes > self.max_response_bytes {
                observation.set_terminal(RequestTerminalState::Error);
                return Err(resource_limit_error("response byte limit exhausted"));
            }
            payload.put_i16(columns as i16);
            payload.put_slice(&projected_row);
            observation.observe_row(row_bytes);
            row_count += 1;
            if payload.len() >= self.max_response_bytes.min(64 * 1024) {
                client
                    .send(PgWireBackendMessage::CopyData(
                        pgwire::messages::copy::CopyData::new(payload.split().freeze()),
                    ))
                    .await?;
                observation.mark_first_row();
            }
        }

        // End-of-copy marker: int16 -1.
        payload.put_i16(-1);

        if !payload.is_empty() {
            client
                .send(PgWireBackendMessage::CopyData(
                    pgwire::messages::copy::CopyData::new(payload.freeze()),
                ))
                .await?;
            if row_count > 0 {
                observation.mark_first_row();
            }
        }

        client
            .send(PgWireBackendMessage::CopyDone(
                pgwire::messages::copy::CopyDone::new(),
            ))
            .await?;

        observation.finish();
        query.skip_next_response_observation();

        Ok(Some(row_count))
    }
}

fn binary_copy_header() -> Bytes {
    // Signature + flags (0) + header extension length (0).
    const SIGNATURE: &[u8] = b"PGCOPY\n\xff\r\n\0";
    let mut buf = BytesMut::with_capacity(SIGNATURE.len() + 8);
    buf.put_slice(SIGNATURE);
    buf.put_i32(0);
    buf.put_i32(0);
    buf.freeze()
}

fn projected_copy_indices(sql: &str, schema: &[FieldInfo]) -> Vec<usize> {
    projection_names(sql)
        .and_then(|names| {
            let schema_names = schema
                .iter()
                .map(|field| field.name().to_lowercase())
                .collect::<Vec<_>>();
            names
                .iter()
                .map(|name| schema_names.iter().position(|field| field == name))
                .collect::<Option<Vec<_>>>()
        })
        .filter(|indices| !indices.is_empty())
        .unwrap_or_else(|| (0..schema.len()).collect())
}

fn projection_names(sql: &str) -> Option<Vec<String>> {
    let dialect = PostgreSqlDialect {};
    let mut statements = Parser::parse_sql(&dialect, sql).ok()?;
    let statement = statements.pop()?;
    let Statement::Query(query) = statement else {
        return None;
    };
    let SetExpr::Select(select) = query.body.as_ref() else {
        return None;
    };

    let mut names = Vec::new();
    for item in &select.projection {
        let name = projection_item_name(item)?;
        if name == "*" {
            return None;
        }
        names.push(name);
    }
    Some(names)
}

fn projection_item_name(item: &SelectItem) -> Option<String> {
    match item {
        SelectItem::UnnamedExpr(expr) => Some(expr_last_identifier(expr)),
        SelectItem::ExprWithAlias { alias, .. } => Some(alias.value.to_lowercase()),
        SelectItem::QualifiedWildcard(_, _) | SelectItem::Wildcard(_) => Some("*".to_string()),
    }
}

fn expr_last_identifier(expr: &Expr) -> String {
    match expr {
        Expr::Identifier(id) => id.value.to_lowercase(),
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .map(|id| id.value.to_lowercase())
            .unwrap_or_default(),
        Expr::Cast { expr, .. } | Expr::Nested(expr) => expr_last_identifier(expr),
        _ => expr.to_string().to_lowercase(),
    }
}

fn project_copy_row_data(
    row_data: &BytesMut,
    field_count: usize,
    projected_indices: &[usize],
    schema: &[FieldInfo],
) -> PgWireResult<BytesMut> {
    let fields = split_copy_row_fields(row_data, field_count)?;
    let mut projected = BytesMut::new();
    for &index in projected_indices {
        let Some(field) = fields.get(index) else {
            return Err(copy_out_error(
                "COPY TO STDOUT projection index out of range",
            ));
        };
        match field {
            Some(value) => {
                let datatype = schema
                    .get(index)
                    .map(|field| field.datatype())
                    .ok_or_else(|| copy_out_error("COPY TO STDOUT schema index out of range"))?;
                let value = encode_binary_copy_field(value, datatype)?;
                projected.put_i32(value.len() as i32);
                projected.put_slice(&value);
            }
            None => projected.put_i32(-1),
        }
    }
    Ok(projected)
}

fn encode_binary_copy_field(value: &[u8], datatype: &Type) -> PgWireResult<Vec<u8>> {
    if datatype == &Type::UUID {
        if value.len() == 16 {
            return Ok(value.to_vec());
        }
        let uuid_text = std::str::from_utf8(value)
            .map_err(|_| copy_out_error("COPY TO STDOUT UUID field is not valid UTF-8"))?;
        let uuid = uuid::Uuid::parse_str(uuid_text)
            .map_err(|_| copy_out_error("COPY TO STDOUT UUID field is invalid"))?;
        return Ok(uuid.as_bytes().to_vec());
    }

    if datatype == &Type::INT8 {
        if value.len() == 8 {
            return Ok(value.to_vec());
        }
        let int_text = std::str::from_utf8(value)
            .map_err(|_| copy_out_error("COPY TO STDOUT INT8 field is not valid UTF-8"))?;
        let value = int_text
            .parse::<i64>()
            .map_err(|_| copy_out_error("COPY TO STDOUT INT8 field is invalid"))?;
        return Ok(value.to_be_bytes().to_vec());
    }

    if datatype == &Type::INT4 {
        if value.len() == 4 {
            return Ok(value.to_vec());
        }
        let int_text = std::str::from_utf8(value)
            .map_err(|_| copy_out_error("COPY TO STDOUT INT4 field is not valid UTF-8"))?;
        let value = int_text
            .parse::<i32>()
            .map_err(|_| copy_out_error("COPY TO STDOUT INT4 field is invalid"))?;
        return Ok(value.to_be_bytes().to_vec());
    }

    if datatype == &Type::INT2 {
        if value.len() == 2 {
            return Ok(value.to_vec());
        }
        let int_text = std::str::from_utf8(value)
            .map_err(|_| copy_out_error("COPY TO STDOUT INT2 field is not valid UTF-8"))?;
        let value = int_text
            .parse::<i16>()
            .map_err(|_| copy_out_error("COPY TO STDOUT INT2 field is invalid"))?;
        return Ok(value.to_be_bytes().to_vec());
    }

    if datatype == &Type::BOOL {
        if value.len() == 1 && (value[0] == 0 || value[0] == 1) {
            return Ok(value.to_vec());
        }
        let bool_text = std::str::from_utf8(value)
            .map_err(|_| copy_out_error("COPY TO STDOUT boolean field is not valid UTF-8"))?
            .to_ascii_lowercase();
        return match bool_text.as_str() {
            "true" | "t" | "1" => Ok(vec![1]),
            "false" | "f" | "0" => Ok(vec![0]),
            _ => Err(copy_out_error("COPY TO STDOUT boolean field is invalid")),
        };
    }

    if datatype == &Type::TIMESTAMP || datatype == &Type::TIMESTAMPTZ {
        if value.len() == 8 {
            return Ok(value.to_vec());
        }
        let timestamp = std::str::from_utf8(value)
            .map_err(|_| copy_out_error("COPY TO STDOUT timestamp field is not valid UTF-8"))?
            .trim();
        const PG_EPOCH_SECONDS: i64 = 946_684_800;
        let micros = if let Ok(unix_seconds) = timestamp.parse::<i64>() {
            unix_seconds
                .saturating_sub(PG_EPOCH_SECONDS)
                .saturating_mul(1_000_000)
        } else if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(timestamp) {
            timestamp.timestamp_micros() - PG_EPOCH_SECONDS * 1_000_000
        } else {
            let timestamp =
                chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S%.f")
                    .or_else(|_| {
                        chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S")
                    })
                    .map_err(|_| copy_out_error("COPY TO STDOUT timestamp field is invalid"))?;
            timestamp.and_utc().timestamp_micros() - PG_EPOCH_SECONDS * 1_000_000
        };
        return Ok(micros.to_be_bytes().to_vec());
    }

    if datatype == &Type::DATE {
        if value.len() == 4 {
            return Ok(value.to_vec());
        }
        let date = std::str::from_utf8(value)
            .map_err(|_| copy_out_error("COPY TO STDOUT date field is not valid UTF-8"))?
            .trim();
        let date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| copy_out_error("COPY TO STDOUT date field is invalid"))?;
        let epoch =
            chrono::NaiveDate::from_ymd_opt(2000, 1, 1).expect("PostgreSQL epoch is a valid date");
        return Ok((date.signed_duration_since(epoch).num_days() as i32)
            .to_be_bytes()
            .to_vec());
    }

    Ok(value.to_vec())
}

fn split_copy_row_fields(
    row_data: &BytesMut,
    field_count: usize,
) -> PgWireResult<Vec<Option<Vec<u8>>>> {
    let mut offset = 0usize;
    let mut fields = Vec::with_capacity(field_count);
    for _ in 0..field_count {
        if row_data.len().saturating_sub(offset) < 4 {
            return Err(copy_out_error(
                "COPY TO STDOUT row field length is truncated",
            ));
        }
        let len = i32::from_be_bytes(
            row_data[offset..offset + 4]
                .try_into()
                .expect("slice length checked"),
        );
        offset += 4;
        if len == -1 {
            fields.push(None);
            continue;
        }
        if len < 0 {
            return Err(copy_out_error(
                "COPY TO STDOUT row contains invalid field length",
            ));
        }
        let len = len as usize;
        if row_data.len().saturating_sub(offset) < len {
            return Err(copy_out_error("COPY TO STDOUT row field data is truncated"));
        }
        fields.push(Some(row_data[offset..offset + len].to_vec()));
        offset += len;
    }
    if offset != row_data.len() {
        return Err(copy_out_error("COPY TO STDOUT row has trailing bytes"));
    }
    Ok(fields)
}

fn copy_out_error(message: &str) -> PgWireError {
    PgWireError::UserError(Box::new(ErrorInfo::new(
        "ERROR".to_string(),
        "XX000".to_string(),
        message.to_string(),
    )))
}

/// Startup handler that enforces authentication when configured.
///
/// When `AuthConfig::is_enabled()` returns false, connections are accepted
/// without any credential check (noop). When it returns true the handler
/// uses either cleartext password auth (constant-time comparison) or
/// SCRAM-SHA-256 (when `AuthConfig::scram_sha256` is set), ensuring
/// credentials are never transmitted in plaintext over the wire.
///
/// When `tls_required` is true and the client connects without TLS, the
/// connection is rejected immediately with a fatal error.
pub struct RockLakeStartupHandler {
    auth: Arc<AuthConfig>,
    tls_required: bool,
    connection: Arc<ConnectionContext>,
    /// Per-connection SCRAM state (None until the client-first-message
    /// is received; Some during the challenge-response phase).
    scram_state: Mutex<Option<crate::scram::ScramState>>,
}

impl RockLakeStartupHandler {
    pub fn new(auth: Arc<AuthConfig>) -> Self {
        Self::new_with_tls_required_and_lifecycle(auth, false, ConnectionContext::standalone(None))
    }

    pub fn new_with_tls_required(auth: Arc<AuthConfig>, tls_required: bool) -> Self {
        Self::new_with_tls_required_and_lifecycle(
            auth,
            tls_required,
            ConnectionContext::standalone(None),
        )
    }

    pub(crate) fn new_with_tls_required_and_lifecycle(
        auth: Arc<AuthConfig>,
        tls_required: bool,
        connection: Arc<ConnectionContext>,
    ) -> Self {
        Self {
            auth,
            tls_required,
            connection,
            scram_state: Mutex::new(None),
        }
    }
}

#[async_trait]
impl pgwire::api::auth::StartupHandler for RockLakeStartupHandler {
    async fn on_startup<C>(
        &self,
        client: &mut C,
        message: PgWireFrontendMessage,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        self.connection.touch();
        match message {
            PgWireFrontendMessage::Startup(ref startup) => {
                // Reject plaintext connections when TLS is required.
                if self.tls_required && !client.is_secure() {
                    let error_info = ErrorInfo::new(
                        "FATAL".to_owned(),
                        "28000".to_owned(),
                        "SSL connection is required. Connect using SSL/TLS.".to_owned(),
                    );
                    client
                        .feed(PgWireBackendMessage::ErrorResponse(ErrorResponse::from(
                            error_info,
                        )))
                        .await?;
                    client.close().await?;
                    return Ok(());
                }

                save_startup_parameters_to_metadata(client, startup);
                if let Some(principal) = client.metadata().get(METADATA_USER) {
                    self.connection.set_principal(principal.clone());
                }
                if let Some(route) = client.metadata().get(METADATA_DATABASE) {
                    self.connection.select_catalog_route(route.clone());
                }
                if !self.auth.is_enabled() {
                    finish_authentication(client, &DefaultServerParameterProvider::default())
                        .await?;
                    self.connection
                        .set_protocol_state(PgWireConnectionState::ReadyForQuery);
                } else if self.auth.scram_sha256 {
                    // Initiate SCRAM-SHA-256 SASL exchange.
                    client.set_state(PgWireConnectionState::AuthenticationInProgress);
                    self.connection
                        .set_protocol_state(PgWireConnectionState::AuthenticationInProgress);
                    client
                        .send(PgWireBackendMessage::Authentication(Authentication::SASL(
                            vec!["SCRAM-SHA-256".to_string()],
                        )))
                        .await?;
                } else {
                    // Cleartext password path: verify username first.
                    let expected_user = self.auth.username.as_deref().unwrap_or("").to_owned();
                    let provided_user = client
                        .metadata()
                        .get(METADATA_USER)
                        .cloned()
                        .unwrap_or_default();
                    if provided_user != expected_user {
                        let error_info = ErrorInfo::new(
                            "FATAL".to_owned(),
                            "28P01".to_owned(),
                            format!("Password authentication failed for user \"{provided_user}\""),
                        );
                        client
                            .feed(PgWireBackendMessage::ErrorResponse(ErrorResponse::from(
                                error_info,
                            )))
                            .await?;
                        client.close().await?;
                        return Ok(());
                    }
                    client.set_state(PgWireConnectionState::AuthenticationInProgress);
                    self.connection
                        .set_protocol_state(PgWireConnectionState::AuthenticationInProgress);
                    client
                        .send(PgWireBackendMessage::Authentication(
                            Authentication::CleartextPassword,
                        ))
                        .await?;
                }
            }

            PgWireFrontendMessage::PasswordMessageFamily(pwd) if self.auth.is_enabled() => {
                if self.auth.scram_sha256 {
                    // Determine whether we are in SCRAM phase 1 (waiting for
                    // SASLInitialResponse) or phase 2 (waiting for SASLResponse).
                    let in_phase1 = self.scram_state.lock().await.is_none();

                    if in_phase1 {
                        // Phase 1 — client-first-message.
                        let initial = match pwd.into_sasl_initial_response() {
                            Ok(r) => r,
                            Err(_) => {
                                return send_auth_error(client, "SCRAM handshake error").await;
                            }
                        };
                        let password = self.auth.password.as_deref().unwrap_or("");
                        let nonce_suffix = crate::scram::random_server_nonce();
                        let client_first = initial.data.as_deref().unwrap_or_default();
                        match crate::scram::ScramState::from_client_first(
                            client_first,
                            password,
                            &nonce_suffix,
                        ) {
                            Some(state) => {
                                let server_first =
                                    Bytes::from(state.server_first.clone().into_bytes());
                                *self.scram_state.lock().await = Some(state);
                                client
                                    .send(PgWireBackendMessage::Authentication(
                                        Authentication::SASLContinue(server_first),
                                    ))
                                    .await?;
                            }
                            None => {
                                return send_auth_error(client, "SCRAM client-first parse error")
                                    .await;
                            }
                        }
                    } else {
                        // Phase 2 — client-final-message.
                        let state = self.scram_state.lock().await.take().unwrap();
                        let response = match pwd.into_sasl_response() {
                            Ok(r) => r,
                            Err(_) => {
                                return send_auth_error(client, "SCRAM handshake error").await;
                            }
                        };
                        match state.validate_client_final(&response.data) {
                            Some(server_final) => {
                                client
                                    .send(PgWireBackendMessage::Authentication(
                                        Authentication::SASLFinal(Bytes::from(server_final)),
                                    ))
                                    .await?;
                                finish_authentication(
                                    client,
                                    &DefaultServerParameterProvider::default(),
                                )
                                .await?;
                                self.connection
                                    .set_protocol_state(PgWireConnectionState::ReadyForQuery);
                            }
                            None => {
                                return send_auth_error(client, "SCRAM authentication failed")
                                    .await;
                            }
                        }
                    }
                } else {
                    // Cleartext password path.
                    let pwd = pwd.into_password()?;
                    let expected = self.auth.password.as_deref().unwrap_or("").as_bytes();
                    if crate::scram::ct_bytes_eq(pwd.password.as_bytes(), expected) {
                        finish_authentication(client, &DefaultServerParameterProvider::default())
                            .await?;
                        self.connection
                            .set_protocol_state(PgWireConnectionState::ReadyForQuery);
                    } else {
                        return send_auth_error(client, "Password authentication failed").await;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Send a `FATAL 28P01` authentication-failure error and close the connection.
async fn send_auth_error<C>(client: &mut C, message: &str) -> PgWireResult<()>
where
    C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send,
    C::Error: Debug,
    PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
{
    let error_info = ErrorInfo::new("FATAL".to_owned(), "28P01".to_owned(), message.to_owned());
    client
        .feed(PgWireBackendMessage::ErrorResponse(ErrorResponse::from(
            error_info,
        )))
        .await?;
    client.close().await?;
    Ok(())
}

impl RockLakeHandler {
    async fn execute_simple_query<'a, C>(
        &self,
        client: &mut C,
        sql: &'a str,
        query: &RequestContext,
    ) -> PgWireResult<(Vec<Response<'a>>, Option<AdmissionPermit>)>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let kind = self.classify_sql(sql, query);
        if let Some(rows) = self.try_stream_copy_to_stdout(client, &kind, query).await? {
            return Ok((
                vec![Response::Execution(
                    pgwire::api::results::Tag::new("COPY").with_rows(rows),
                )],
                None,
            ));
        }

        let scan_permit = self.acquire_scan(&kind, query).await?;
        let params = ParamValues::default();
        let execution_started = Instant::now();
        let session_handle = self.connection.session();
        let mut session = session_handle.lock().await;
        let result = tokio::select! {
            result = executor::execute_sql_with_mode(
                sql,
                &params,
                &self.catalog,
                &mut session,
                &self.notify_manager,
                &self.extension_schemas,
                self.access_mode,
            ) => result,
            _ = query.cancelled() => {
                query.record_execution(execution_started);
                return Err(query.cancellation_error());
            }
        };
        query.record_execution(execution_started);
        Ok((
            result.map_err(|error| -> PgWireError { error.into() })?,
            scan_permit,
        ))
    }

    async fn execute_extended_query<'a, C>(
        &self,
        client: &mut C,
        portal: &'a Portal<String>,
        query: &RequestContext,
    ) -> PgWireResult<(Option<Response<'a>>, Option<AdmissionPermit>)>
    where
        C: ClientInfo + ClientPortalStore + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::PortalStore: PortalStore<Statement = String>,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let sql = &portal.statement.statement;
        let kind = self.classify_sql(sql, query);
        if let Some(rows) = self.try_stream_copy_to_stdout(client, &kind, query).await? {
            return Ok((
                Some(Response::Execution(
                    pgwire::api::results::Tag::new("COPY").with_rows(rows),
                )),
                None,
            ));
        }

        let scan_permit = self.acquire_scan(&kind, query).await?;

        // Extract parameters from the portal, including binary-encoded integers.
        let inferred_types = describe_params_for_sql(sql);
        let param_values: Vec<Option<String>> = portal
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| {
                p.as_ref().map(|b| {
                    if portal.parameter_format.is_binary(i) {
                        let pg_type = portal
                            .statement
                            .parameter_types
                            .get(i)
                            .filter(|t| **t != Type::UNKNOWN)
                            .cloned()
                            .or_else(|| inferred_types.get(i).cloned())
                            .unwrap_or(Type::UNKNOWN);
                        match pg_type {
                            Type::INT8 if b.len() == 8 => {
                                let bytes: [u8; 8] = b[..8].try_into().unwrap_or([0; 8]);
                                return i64::from_be_bytes(bytes).to_string();
                            }
                            Type::INT4 if b.len() == 4 => {
                                let bytes: [u8; 4] = b[..4].try_into().unwrap_or([0; 4]);
                                return i32::from_be_bytes(bytes).to_string();
                            }
                            Type::INT2 if b.len() == 2 => {
                                let bytes: [u8; 2] = b[..2].try_into().unwrap_or([0; 2]);
                                return i16::from_be_bytes(bytes).to_string();
                            }
                            _ => {}
                        }
                    }
                    String::from_utf8_lossy(b).to_string()
                })
            })
            .collect();
        let params = ParamValues::new(param_values);

        let execution_started = Instant::now();
        let session_handle = self.connection.session();
        let mut session = session_handle.lock().await;
        let result = tokio::select! {
            result = executor::execute_sql_with_mode(
                sql,
                &params,
                &self.catalog,
                &mut session,
                &self.notify_manager,
                &self.extension_schemas,
                self.access_mode,
            ) => result,
            _ = query.cancelled() => {
                query.record_execution(execution_started);
                return Err(query.cancellation_error());
            }
        };
        query.record_execution(execution_started);
        let mut responses = result.map_err(|error| -> PgWireError { error.into() })?;
        Ok((responses.pop(), scan_permit))
    }

    async fn send_query_response_with_telemetry<'a, C>(
        &self,
        client: &mut C,
        results: QueryResponse<'a>,
        send_describe: bool,
        query: &RequestContext,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let mut observation = query.response_observer();
        let row_schema = results.row_schema();
        if send_describe {
            client
                .send(PgWireBackendMessage::RowDescription(RowDescription::new(
                    row_schema.iter().map(Into::into).collect(),
                )))
                .await?;
        }

        let command_tag = results.command_tag().to_owned();
        let mut rows = results.data_rows();
        let mut row_count = 0u64;
        let mut response_bytes = 0u64;
        while let Some(row_result) = rows.next().await {
            if query.is_cancelled() {
                observation.set_terminal(RequestTerminalState::Cancelled);
                return Err(query.cancellation_error());
            }
            let row = match row_result {
                Ok(row) => row,
                Err(error) => {
                    observation.set_terminal(RequestTerminalState::Error);
                    return Err(error);
                }
            };
            if self.max_buffered_rows < 1 {
                if let Some(metrics) = query.metrics() {
                    metrics.increment_resource_limit_exhaustions();
                }
                observation.set_terminal(RequestTerminalState::Error);
                return Err(resource_limit_error("buffered row limit exhausted"));
            }
            let _buffer_permit = match self.response_buffer.clone().try_acquire_owned() {
                Ok(permit) => AdmissionPermit::response_buffer(permit),
                Err(_) => {
                    if let Some(metrics) = query.metrics() {
                        metrics.increment_resource_limit_exhaustions();
                    }
                    observation.set_terminal(RequestTerminalState::Error);
                    return Err(resource_limit_error("response buffer capacity exhausted"));
                }
            };
            let next_bytes = response_bytes.saturating_add(row.data.len() as u64);
            if next_bytes > self.max_response_bytes as u64 {
                if let Some(metrics) = query.metrics() {
                    metrics.increment_resource_limit_exhaustions();
                }
                observation.set_terminal(RequestTerminalState::Error);
                return Err(resource_limit_error("response byte limit exhausted"));
            }
            response_bytes = next_bytes;
            row_count += 1;
            observation.observe_row(row.data.len());
            client.feed(PgWireBackendMessage::DataRow(row)).await?;
            if row_count == 1 {
                client.flush().await?;
                observation.mark_first_row();
            }
        }

        client
            .send(PgWireBackendMessage::CommandComplete(
                CommandComplete::from(
                    pgwire::api::results::Tag::new(&command_tag).with_rows(row_count as usize),
                ),
            ))
            .await?;
        observation.finish();
        Ok(())
    }

    async fn send_response<'a, C>(
        &self,
        client: &mut C,
        response: Response<'a>,
        send_describe: bool,
        extended: bool,
        transaction_status: &mut pgwire::messages::response::TransactionStatus,
        query: &RequestContext,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        if let Response::Query(results) = response {
            return self
                .send_query_response_with_telemetry(client, results, send_describe, query)
                .await;
        }
        let response_is_error = matches!(response, Response::Error(_));
        let mut observation = if query.take_skipped_response_observation() {
            None
        } else {
            Some(query.response_observer())
        };
        match response {
            Response::Query(_) => unreachable!("query responses are handled above"),
            Response::EmptyQuery => {
                client
                    .feed(PgWireBackendMessage::EmptyQueryResponse(
                        EmptyQueryResponse::new(),
                    ))
                    .await?;
            }
            Response::Execution(tag) => {
                send_execution_response(client, tag).await?;
            }
            Response::TransactionStart(tag) => {
                send_execution_response(client, tag).await?;
                *transaction_status = transaction_status.to_in_transaction_state();
                self.connection.set_transaction_status(*transaction_status);
            }
            Response::TransactionEnd(tag) => {
                send_execution_response(client, tag).await?;
                *transaction_status = transaction_status.to_idle_state();
                self.connection.set_transaction_status(*transaction_status);
            }
            Response::Error(error) => {
                query.record_error_info(&error);
                client
                    .feed(PgWireBackendMessage::ErrorResponse((*error).into()))
                    .await?;
                *transaction_status = transaction_status.to_error_state();
                self.connection.set_transaction_status(*transaction_status);
                if let Some(observation) = &mut observation {
                    observation.set_terminal(RequestTerminalState::Error);
                }
            }
            Response::CopyIn(result) => {
                client.set_state(PgWireConnectionState::CopyInProgress(extended));
                self.connection
                    .set_protocol_state(PgWireConnectionState::CopyInProgress(extended));
                pgwire::api::copy::send_copy_in_response(client, result).await?;
            }
            Response::CopyOut(result) => {
                client.set_state(PgWireConnectionState::CopyInProgress(extended));
                self.connection
                    .set_protocol_state(PgWireConnectionState::CopyInProgress(extended));
                pgwire::api::copy::send_copy_out_response(client, result).await?;
            }
            Response::CopyBoth(result) => {
                client.set_state(PgWireConnectionState::CopyInProgress(extended));
                self.connection
                    .set_protocol_state(PgWireConnectionState::CopyInProgress(extended));
                pgwire::api::copy::send_copy_both_response(client, result).await?;
            }
        }
        if let Some(observation) = observation {
            observation.finish();
        }
        if response_is_error {
            query.finish(RequestTerminalState::Error);
        }
        Ok(())
    }
}

#[async_trait]
impl SimpleQueryHandler for RockLakeHandler {
    async fn on_query<C>(
        &self,
        client: &mut C,
        message: pgwire::messages::simplequery::Query,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        if self.connection.is_draining() {
            return Err(server_shutting_down_error());
        }
        let query = self.request_context(OperationClass::Interactive);
        let result = async {
            if !matches!(client.state(), PgWireConnectionState::ReadyForQuery) {
                return Err(PgWireError::NotReadyForQuery);
            }
            let mut transaction_status = client.transaction_status();
            client.set_state(PgWireConnectionState::QueryInProgress);
            self.connection
                .set_protocol_state(PgWireConnectionState::QueryInProgress);
            let sql = message.query;
            if sql.trim().is_empty() || sql.trim() == ";" {
                let observation = query.response_observer();
                client
                    .feed(PgWireBackendMessage::EmptyQueryResponse(
                        EmptyQueryResponse::new(),
                    ))
                    .await?;
                observation.finish();
            } else {
                let (responses, permit) = self.execute_simple_query(client, &sql, &query).await?;
                for response in responses {
                    self.send_response(
                        client,
                        response,
                        true,
                        false,
                        &mut transaction_status,
                        &query,
                    )
                    .await?;
                }
                drop(permit);
            }
            if !matches!(client.state(), PgWireConnectionState::CopyInProgress(_)) {
                client.set_state(PgWireConnectionState::ReadyForQuery);
                client.set_transaction_status(transaction_status);
                self.connection
                    .set_protocol_state(PgWireConnectionState::ReadyForQuery);
                self.connection.set_transaction_status(transaction_status);
                send_ready_for_query(client, transaction_status).await?;
            }
            query.finish(RequestTerminalState::Completed);
            Ok(())
        }
        .instrument(query.span())
        .await;
        if let Err(error) = &result {
            query.record_error(error);
            query.finish(RequestTerminalState::Error);
        }
        result
    }

    async fn do_query<'a, 'b: 'a, C>(
        &'b self,
        client: &mut C,
        query: &'a str,
    ) -> PgWireResult<Vec<Response<'a>>>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let telemetry = self.request_context(OperationClass::Interactive);
        let result = async {
            let (responses, mut permit) =
                self.execute_simple_query(client, query, &telemetry).await?;
            let has_query = responses
                .iter()
                .any(|response| matches!(response, Response::Query(_)));
            let has_error = responses
                .iter()
                .any(|response| matches!(response, Response::Error(_)));
            let responses = responses
                .into_iter()
                .map(|response| {
                    wrap_query_response(
                        response,
                        permit.take(),
                        self.max_buffered_rows,
                        self.max_response_bytes,
                        self.response_buffer.clone(),
                        telemetry.clone(),
                    )
                })
                .collect();
            if !has_query {
                telemetry.finish(if has_error {
                    RequestTerminalState::Error
                } else {
                    RequestTerminalState::Completed
                });
            }
            Ok(responses)
        }
        .instrument(telemetry.span())
        .await;
        if let Err(error) = &result {
            telemetry.record_error(error);
            telemetry.finish(RequestTerminalState::Error);
        }
        result
    }
}

/// Query parser that stores SQL strings.
#[derive(Debug, Clone)]
pub struct RockLakeQueryParser;

#[async_trait]
impl QueryParser for RockLakeQueryParser {
    type Statement = String;

    async fn parse_sql(&self, sql: &str, _types: &[Type]) -> PgWireResult<Self::Statement> {
        Ok(sql.to_owned())
    }
}

#[async_trait]
impl ExtendedQueryHandler for RockLakeHandler {
    type Statement = String;
    type QueryParser = RockLakeQueryParser;

    fn query_parser(&self) -> Arc<Self::QueryParser> {
        self.parser.clone()
    }

    async fn on_execute<C>(
        &self,
        client: &mut C,
        message: pgwire::messages::extendedquery::Execute,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + ClientPortalStore + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::PortalStore: PortalStore<Statement = Self::Statement>,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        if self.connection.is_draining() {
            return Err(server_shutting_down_error());
        }
        let query = self.request_context(OperationClass::Interactive);
        let result = async {
            if !matches!(client.state(), PgWireConnectionState::ReadyForQuery) {
                return Err(PgWireError::NotReadyForQuery);
            }
            let mut transaction_status = client.transaction_status();
            client.set_state(PgWireConnectionState::QueryInProgress);
            self.connection
                .set_protocol_state(PgWireConnectionState::QueryInProgress);
            let portal_name = message.name.as_deref().unwrap_or(pgwire::api::DEFAULT_NAME);
            let portal = client
                .portal_store()
                .get_portal(portal_name)
                .ok_or_else(|| PgWireError::PortalNotFound(portal_name.to_owned()))?;
            let (response, permit) = self.execute_extended_query(client, &portal, &query).await?;
            if let Some(response) = response {
                self.send_response(
                    client,
                    response,
                    false,
                    true,
                    &mut transaction_status,
                    &query,
                )
                .await?;
            }
            drop(permit);
            if !matches!(client.state(), PgWireConnectionState::CopyInProgress(_)) {
                client.set_state(PgWireConnectionState::ReadyForQuery);
                client.set_transaction_status(transaction_status);
                self.connection
                    .set_protocol_state(PgWireConnectionState::ReadyForQuery);
                self.connection.set_transaction_status(transaction_status);
            }
            query.finish(RequestTerminalState::Completed);
            Ok(())
        }
        .instrument(query.span())
        .await;
        if let Err(error) = &result {
            query.record_error(error);
            query.finish(RequestTerminalState::Error);
        }
        result
    }

    async fn do_query<'a, 'b: 'a, C>(
        &'b self,
        client: &mut C,
        portal: &'a Portal<Self::Statement>,
        _max_rows: usize,
    ) -> PgWireResult<Response<'a>>
    where
        C: ClientInfo + ClientPortalStore + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::PortalStore: PortalStore<Statement = Self::Statement>,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let telemetry = self.request_context(OperationClass::Interactive);
        let result = async {
            let (response, mut permit) = self
                .execute_extended_query(client, portal, &telemetry)
                .await?;
            let Some(response) = response else {
                telemetry.finish(RequestTerminalState::Completed);
                return Ok(Response::EmptyQuery);
            };
            let response = wrap_query_response(
                response,
                permit.take(),
                self.max_buffered_rows,
                self.max_response_bytes,
                self.response_buffer.clone(),
                telemetry.clone(),
            );
            if permit.is_none() && !matches!(response, Response::Query(_)) {
                telemetry.finish(RequestTerminalState::Completed);
            }
            Ok(response)
        }
        .instrument(telemetry.span())
        .await;
        if let Err(error) = &result {
            telemetry.record_error(error);
            telemetry.finish(RequestTerminalState::Error);
        }
        result
    }

    async fn do_describe_statement<C>(
        &self,
        _client: &mut C,
        stmt: &StoredStatement<Self::Statement>,
    ) -> PgWireResult<DescribeStatementResponse>
    where
        C: ClientInfo + ClientPortalStore + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::PortalStore: PortalStore<Statement = Self::Statement>,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let sql = &stmt.statement;
        let fields = describe_fields_for_sql_with_catalog(sql, &self.catalog).await;

        // Return precise parameter types so the client can correctly serialize
        // typed values (e.g. i64 → INT8). When the client provided type hints in
        // its Parse message we respect those; otherwise we infer from the
        // StatementKind.
        let param_types = if !stmt.parameter_types.is_empty()
            && stmt.parameter_types.iter().any(|t| *t != Type::UNKNOWN)
        {
            stmt.parameter_types.clone()
        } else {
            describe_params_for_sql(sql)
        };

        Ok(DescribeStatementResponse::new(param_types, fields))
    }

    async fn do_describe_portal<C>(
        &self,
        _client: &mut C,
        portal: &Portal<Self::Statement>,
    ) -> PgWireResult<DescribePortalResponse>
    where
        C: ClientInfo + ClientPortalStore + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::PortalStore: PortalStore<Statement = Self::Statement>,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let sql = &portal.statement.statement;
        let fields = describe_fields_for_sql_with_catalog(sql, &self.catalog).await;
        Ok(DescribePortalResponse::new(fields))
    }
}

/// Server handlers collection for RockLake.
pub struct RockLakeServerHandlers {
    pub handler: Arc<RockLakeHandler>,
    pub startup: Arc<RockLakeStartupHandler>,
    pub copy_handler: Arc<RockLakeCopyHandler>,
    pub error_handler: Arc<NoopErrorHandler>,
}

impl RockLakeServerHandlers {
    pub fn new(catalog: Arc<Mutex<CatalogStore>>) -> Self {
        let auth = Arc::new(AuthConfig::default());
        let handler = Arc::new(RockLakeHandler::new(catalog));
        let connection = handler.connection.clone();
        let copy_handler = Arc::new(RockLakeCopyHandler::new_with_connection(
            connection.clone(),
            handler.access_mode,
        ));
        Self {
            handler,
            startup: Arc::new(RockLakeStartupHandler::new_with_tls_required_and_lifecycle(
                auth, false, connection,
            )),
            copy_handler,
            error_handler: Arc::new(NoopErrorHandler),
        }
    }

    pub fn new_with_auth(catalog: Arc<Mutex<CatalogStore>>, auth: Arc<AuthConfig>) -> Self {
        let handler = Arc::new(RockLakeHandler::new_with_auth(catalog, auth.clone()));
        let connection = handler.connection.clone();
        let copy_handler = Arc::new(RockLakeCopyHandler::new_with_connection(
            connection.clone(),
            handler.access_mode,
        ));
        Self {
            handler,
            startup: Arc::new(RockLakeStartupHandler::new_with_tls_required_and_lifecycle(
                auth, false, connection,
            )),
            copy_handler,
            error_handler: Arc::new(NoopErrorHandler),
        }
    }

    pub fn new_with_config(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        tls_required: bool,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
    ) -> Self {
        Self::new_with_config_mode(
            catalog,
            auth.clone(),
            tls_required,
            notify_manager,
            extension_schemas,
            executor::AccessMode::Writer,
        )
    }

    pub fn new_with_config_mode(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        tls_required: bool,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
        access_mode: executor::AccessMode,
    ) -> Self {
        let handler = Arc::new(RockLakeHandler::new_with_config_mode(
            catalog,
            auth.clone(),
            notify_manager,
            extension_schemas,
            access_mode,
        ));
        let connection = handler.connection.clone();
        let copy_handler = Arc::new(RockLakeCopyHandler::new_with_connection(
            connection.clone(),
            access_mode,
        ));
        Self {
            handler,
            startup: Arc::new(RockLakeStartupHandler::new_with_tls_required_and_lifecycle(
                auth,
                tls_required,
                connection,
            )),
            copy_handler,
            error_handler: Arc::new(NoopErrorHandler),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_config_mode_and_limits(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        tls_required: bool,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
        access_mode: executor::AccessMode,
        scan_semaphore: Arc<Semaphore>,
        max_active_scans: usize,
        max_buffered_rows: usize,
        max_response_bytes: usize,
        slow_operation_threshold: Duration,
        metrics: Option<Arc<CatalogMetrics>>,
    ) -> Self {
        Self::new_with_config_mode_and_limits_and_lifecycle(
            catalog,
            auth.clone(),
            tls_required,
            notify_manager,
            extension_schemas,
            access_mode,
            scan_semaphore,
            max_active_scans,
            max_buffered_rows,
            max_response_bytes,
            slow_operation_threshold,
            ConnectionContext::standalone(metrics),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_config_mode_and_limits_and_lifecycle(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        tls_required: bool,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
        access_mode: executor::AccessMode,
        scan_semaphore: Arc<Semaphore>,
        max_active_scans: usize,
        max_buffered_rows: usize,
        max_response_bytes: usize,
        slow_operation_threshold: Duration,
        connection: Arc<ConnectionContext>,
    ) -> Self {
        let handler = Arc::new(
            RockLakeHandler::new_with_config_mode_and_limits_and_lifecycle(
                catalog,
                auth.clone(),
                notify_manager,
                extension_schemas,
                access_mode,
                scan_semaphore,
                max_active_scans,
                max_buffered_rows,
                max_response_bytes,
                slow_operation_threshold,
                connection.clone(),
            ),
        );
        let copy_handler = Arc::new(RockLakeCopyHandler::new_with_connection(
            connection.clone(),
            access_mode,
        ));
        Self {
            handler,
            startup: Arc::new(RockLakeStartupHandler::new_with_tls_required_and_lifecycle(
                auth,
                tls_required,
                connection,
            )),
            copy_handler,
            error_handler: Arc::new(NoopErrorHandler),
        }
    }
}

impl PgWireServerHandlers for RockLakeServerHandlers {
    type StartupHandler = RockLakeStartupHandler;
    type SimpleQueryHandler = RockLakeHandler;
    type ExtendedQueryHandler = RockLakeHandler;
    type CopyHandler = RockLakeCopyHandler;
    type ErrorHandler = NoopErrorHandler;

    fn simple_query_handler(&self) -> Arc<Self::SimpleQueryHandler> {
        self.handler.clone()
    }

    fn extended_query_handler(&self) -> Arc<Self::ExtendedQueryHandler> {
        self.handler.clone()
    }

    fn startup_handler(&self) -> Arc<Self::StartupHandler> {
        self.startup.clone()
    }

    fn copy_handler(&self) -> Arc<Self::CopyHandler> {
        self.copy_handler.clone()
    }

    fn error_handler(&self) -> Arc<Self::ErrorHandler> {
        self.error_handler.clone()
    }
}

/// Count the number of positional parameters (`$1`, `$2`, …) in a SQL string.
/// Returns the highest parameter index found, which equals the number of
/// parameters the client must bind.
fn count_sql_params(sql: &str) -> usize {
    let mut max = 0usize;
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > start {
                if let Ok(n) = sql[start..end].parse::<usize>() {
                    if n > max {
                        max = n;
                    }
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }
    max
}

/// Return the expected parameter types for a SQL statement.
/// Allows tokio-postgres to correctly serialize typed Rust values (e.g. i64→INT8)
/// even when the client sends no type hints in the Parse message.
fn describe_params_for_sql(sql: &str) -> Vec<Type> {
    use rocklake_sql::StatementKind;
    let kind =
        rocklake_sql::classify_statement(sql).unwrap_or(StatementKind::Unsupported(String::new()));
    match kind {
        // Snapshot-scoped catalog selects: $1 = snapshot_id (INT8)
        StatementKind::SelectSchemas
        | StatementKind::SelectTables
        | StatementKind::SelectDataFiles
        | StatementKind::SelectMaxSnapshotAfter => vec![Type::INT8],
        // Inserts whose first column is a numeric FK
        StatementKind::InsertTable => vec![Type::INT8, Type::TEXT, Type::TEXT],
        StatementKind::InsertDataFile => {
            // table_id, path, format, row_count, file_size_bytes
            vec![Type::INT8, Type::TEXT, Type::TEXT, Type::INT8, Type::INT8]
        }
        // Text-only inserts
        StatementKind::InsertSchema => vec![Type::TEXT],
        StatementKind::InsertSnapshot => vec![Type::TEXT, Type::TEXT],
        // table_changes(table_name TEXT, from_snapshot INT8, to_snapshot INT8)
        StatementKind::TableChanges { .. } => vec![Type::TEXT, Type::INT8, Type::INT8],
        // Everything else: fall back to UNKNOWN (works for &str / String)
        _ => {
            let count = count_sql_params(sql);
            vec![Type::UNKNOWN; count]
        }
    }
}

/// Return the result-set field descriptions for a SQL statement.
/// Used by both `do_describe_statement` and `do_describe_portal`.
async fn describe_fields_for_sql_with_catalog(
    sql: &str,
    catalog: &Arc<Mutex<CatalogStore>>,
) -> Vec<pgwire::api::results::FieldInfo> {
    let kind = rocklake_sql::classify_statement(sql)
        .unwrap_or(rocklake_sql::StatementKind::Unsupported(String::new()));
    if matches!(kind, rocklake_sql::StatementKind::SelectInlinedRows) {
        if let Some((table_id, _schema_version)) = parse_inlined_table_ids_from_sql(sql) {
            let reader = { catalog.lock().await.read_latest() };
            if let Ok(Some((_, columns))) = reader.describe_table(table_id).await {
                return describe_inlined_row_fields(sql, &columns);
            }
        }
    }
    describe_fields_for_sql(sql)
}

fn describe_fields_for_sql(sql: &str) -> Vec<pgwire::api::results::FieldInfo> {
    use pgwire::api::results::{FieldFormat, FieldInfo};

    let kind = rocklake_sql::classify_statement(sql)
        .unwrap_or(rocklake_sql::StatementKind::Unsupported(String::new()));

    macro_rules! text_col {
        ($name:expr) => {
            FieldInfo::new($name.to_string(), None, None, Type::TEXT, FieldFormat::Text)
        };
    }
    macro_rules! int8_col {
        ($name:expr) => {
            FieldInfo::new(
                $name.to_string(),
                None,
                None,
                Type::INT8,
                FieldFormat::Binary,
            )
        };
    }

    match kind {
        rocklake_sql::StatementKind::SelectVersion => vec![text_col!("version")],
        rocklake_sql::StatementKind::SelectOne => vec![int8_col!("?column?")],
        rocklake_sql::StatementKind::SelectCurrentSchema => vec![text_col!("current_schema")],
        rocklake_sql::StatementKind::SelectCurrentDatabase => {
            vec![text_col!("current_database")]
        }
        rocklake_sql::StatementKind::SelectPgType => vec![
            FieldInfo::new("oid".to_string(), None, None, Type::INT4, FieldFormat::Text),
            text_col!("typname"),
        ],
        rocklake_sql::StatementKind::SelectMaxSnapshot
        | rocklake_sql::StatementKind::SelectMaxSnapshotAfter => {
            vec![int8_col!("max")]
        }
        rocklake_sql::StatementKind::SelectLatestSnapshotId => {
            vec![int8_col!("ducklake_latest_snapshot_id")]
        }
        // Delegate to registry for the 4-col latest snapshot info shape.
        rocklake_sql::StatementKind::SelectLatestSnapshotInfo => {
            (*crate::schema_registry::latest_snapshot_info_schema()).clone()
        }
        rocklake_sql::StatementKind::ShowVariable(ref var) => {
            vec![text_col!(var.as_str())]
        }
        // ── Catalog table schemas — all derived from the shared schema registry ──
        rocklake_sql::StatementKind::SelectSchemas => {
            project_described_fields(sql, (*crate::schema_registry::schema_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectTables => {
            project_described_fields(sql, (*crate::schema_registry::table_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectColumns => {
            project_described_fields(sql, (*crate::schema_registry::column_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectDataFiles => {
            project_described_fields(sql, (*crate::schema_registry::data_file_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectFileColumnStats => project_described_fields(
            sql,
            (*crate::schema_registry::file_column_stats_schema()).clone(),
        ),
        // Combined global stats query (table_stats joined with column_stats).
        rocklake_sql::StatementKind::SelectTableStats
            if sql
                .to_ascii_lowercase()
                .contains("ducklake_table_column_stats") =>
        {
            project_described_fields(
                sql,
                (*crate::schema_registry::global_table_stats_schema()).clone(),
            )
        }
        rocklake_sql::StatementKind::SelectTableStats => {
            project_described_fields(sql, (*crate::schema_registry::table_stats_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectTableColumnStats => project_described_fields(
            sql,
            (*crate::schema_registry::table_column_stats_schema()).clone(),
        ),
        rocklake_sql::StatementKind::SelectInlinedData => project_described_fields(
            sql,
            if sql.to_ascii_lowercase().contains("ctid") {
                vec![
                    int8_col!("table_id"),
                    int8_col!("schema_version"),
                    FieldInfo::new("ctid".into(), None, None, Type::TID, FieldFormat::Binary),
                ]
            } else {
                (*crate::schema_registry::inlined_data_tables_schema()).clone()
            },
        ),
        // ── Additional catalog table schemas from registry ─────────────────
        rocklake_sql::StatementKind::SelectSnapshot
        | rocklake_sql::StatementKind::SelectFirstSnapshot => {
            project_described_fields(sql, (*crate::schema_registry::snapshot_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectSnapshotChanges => project_described_fields(
            sql,
            (*crate::schema_registry::snapshot_changes_schema()).clone(),
        ),
        rocklake_sql::StatementKind::SelectDeleteFiles => {
            project_described_fields(sql, (*crate::schema_registry::delete_file_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectViews => {
            project_described_fields(sql, (*crate::schema_registry::view_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectMacros => {
            project_described_fields(sql, (*crate::schema_registry::macro_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectMacroImpls => {
            project_described_fields(sql, (*crate::schema_registry::macro_impl_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectMacroParameters => project_described_fields(
            sql,
            (*crate::schema_registry::macro_parameters_schema()).clone(),
        ),
        rocklake_sql::StatementKind::SelectTags => {
            project_described_fields(sql, (*crate::schema_registry::tag_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectColumnTags => {
            project_described_fields(sql, (*crate::schema_registry::column_tag_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectSortInfo => {
            project_described_fields(sql, (*crate::schema_registry::sort_info_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectMetadata => {
            project_described_fields(sql, (*crate::schema_registry::metadata_schema()).clone())
        }
        rocklake_sql::StatementKind::SelectSchemaVersion => project_described_fields(
            sql,
            (*crate::schema_registry::schema_version_schema()).clone(),
        ),
        rocklake_sql::StatementKind::SelectDuckLakeMetadataTable { ref table_name } => {
            if let Some(schema) = crate::schema_registry::fields_for_table(table_name) {
                project_described_fields(sql, (*schema).clone())
            } else {
                vec![]
            }
        }
        rocklake_sql::StatementKind::VirtualCatalogScan { ref table_name } => {
            if let Some(schema) = crate::schema_registry::fields_for_table(table_name) {
                project_described_fields(sql, (*schema).clone())
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

fn parse_inlined_table_ids_from_sql(sql: &str) -> Option<(u64, u64)> {
    let lower = sql.to_ascii_lowercase();
    let start = lower.find("ducklake_inlined_data_")?;
    let rest = &lower[start + "ducklake_inlined_data_".len()..];
    let mut parts = rest
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()?
        .split('_');
    let table_id = parts.next()?.parse().ok()?;
    let schema_version = parts.next()?.parse().ok()?;
    Some((table_id, schema_version))
}

fn describe_inlined_row_fields(
    sql: &str,
    columns: &[ColumnRow],
) -> Vec<pgwire::api::results::FieldInfo> {
    use pgwire::api::results::{FieldFormat, FieldInfo};

    let field_for_name = |name: &str| {
        let lower = name.trim_matches('"').to_ascii_lowercase();
        match lower.as_str() {
            "row_id" => Some(FieldInfo::new(
                name.to_string(),
                None,
                None,
                Type::INT8,
                FieldFormat::Binary,
            )),
            "begin_snapshot" => Some(FieldInfo::new(
                name.to_string(),
                None,
                None,
                Type::INT8,
                FieldFormat::Binary,
            )),
            "end_snapshot" => Some(FieldInfo::new(
                name.to_string(),
                None,
                None,
                Type::INT8,
                FieldFormat::Binary,
            )),
            "ctid" => Some(FieldInfo::new(
                name.to_string(),
                None,
                None,
                Type::TID,
                FieldFormat::Binary,
            )),
            _ => columns
                .iter()
                .find(|column| column.column_name.eq_ignore_ascii_case(&lower))
                .map(|column| {
                    FieldInfo::new(
                        name.to_string(),
                        None,
                        None,
                        inlined_storage_type(&column.data_type),
                        FieldFormat::Binary,
                    )
                }),
        }
    };

    let Some(names) = projection_names(sql) else {
        return columns
            .iter()
            .map(|column| {
                FieldInfo::new(
                    column.column_name.clone(),
                    None,
                    None,
                    inlined_storage_type(&column.data_type),
                    FieldFormat::Binary,
                )
            })
            .collect();
    };
    let fields = names
        .iter()
        .filter_map(|name| field_for_name(name))
        .collect::<Vec<_>>();
    if fields.is_empty() {
        describe_fields_for_sql(sql)
    } else {
        fields
    }
}

fn inlined_storage_type(logical_type: &str) -> Type {
    match logical_type.to_ascii_uppercase().as_str() {
        "BOOLEAN" | "BOOL" => Type::BOOL,
        "TINYINT" | "SMALLINT" | "INT2" | "INT16" => Type::INT2,
        "INTEGER" | "INT" | "INT4" | "INT32" => Type::INT4,
        "BIGINT" | "INT8" | "INT64" => Type::INT8,
        "VARCHAR" | "TEXT" | "STRING" | "BLOB" | "BYTEA" => Type::BYTEA,
        "TIMESTAMP" | "TIMESTAMP WITHOUT TIME ZONE" => Type::TIMESTAMP,
        "TIMESTAMP WITH TIME ZONE" | "TIMESTAMPTZ" => Type::TIMESTAMPTZ,
        "DATE" => Type::DATE,
        _ => Type::TEXT,
    }
}

fn project_described_fields(
    _sql: &str,
    schema: Vec<pgwire::api::results::FieldInfo>,
) -> Vec<pgwire::api::results::FieldInfo> {
    // The execute path (make_*_response) always returns ALL catalog columns
    // regardless of the SELECT projection.  Returning the full schema here
    // ensures the RowDescription from Describe matches the DataRow from
    // Execute, so clients can resolve columns by name rather than by position.
    schema
}
