//! PG Wire protocol handler implementation.
//!
//! Implements SimpleQueryHandler and ExtendedQueryHandler for the pgwire crate.
//! Supports optional password authentication (cleartext with constant-time comparison).

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use futures::sink::{Sink, SinkExt};
use pgwire::api::auth::{
    finish_authentication, save_startup_parameters_to_metadata, DefaultServerParameterProvider,
};
use pgwire::api::copy::NoopCopyHandler;
use pgwire::api::portal::Portal;
use pgwire::api::query::{ExtendedQueryHandler, SimpleQueryHandler};
use pgwire::api::results::{DescribePortalResponse, DescribeStatementResponse, Response};
use pgwire::api::stmt::{QueryParser, StoredStatement};
use pgwire::api::store::PortalStore;
use pgwire::api::{
    ClientInfo, ClientPortalStore, NoopErrorHandler, PgWireConnectionState, PgWireServerHandlers,
    Type, METADATA_USER,
};
use pgwire::error::{ErrorInfo, PgWireError, PgWireResult};
use pgwire::messages::response::ErrorResponse;
use pgwire::messages::startup::Authentication;
use pgwire::messages::{PgWireBackendMessage, PgWireFrontendMessage};
use tokio::sync::Mutex;

use slateduck_catalog::CatalogStore;
use slateduck_sql::ParamValues;

use crate::executor;
use crate::notify::NotifyManager;
use crate::server::AuthConfig;
use crate::session::SessionState;

/// The main SlateDuck query handler.
pub struct SlateDuckHandler {
    pub catalog: Arc<Mutex<CatalogStore>>,
    pub session: Arc<Mutex<SessionState>>,
    pub parser: Arc<SlateDuckQueryParser>,
    pub auth: Arc<AuthConfig>,
    /// Shared LISTEN/NOTIFY manager for this server instance.
    pub notify_manager: Arc<NotifyManager>,
    /// Allowed extension schema names (configurable via --extension-schemas).
    pub extension_schemas: Arc<Vec<String>>,
}

impl SlateDuckHandler {
    pub fn new(catalog: Arc<Mutex<CatalogStore>>) -> Self {
        Self {
            catalog,
            session: Arc::new(Mutex::new(SessionState::new())),
            parser: Arc::new(SlateDuckQueryParser),
            auth: Arc::new(AuthConfig::default()),
            notify_manager: Arc::new(NotifyManager::new()),
            extension_schemas: Arc::new(vec!["pgtrickle".to_string()]),
        }
    }

    pub fn new_with_auth(catalog: Arc<Mutex<CatalogStore>>, auth: Arc<AuthConfig>) -> Self {
        Self {
            catalog,
            session: Arc::new(Mutex::new(SessionState::new())),
            parser: Arc::new(SlateDuckQueryParser),
            auth,
            notify_manager: Arc::new(NotifyManager::new()),
            extension_schemas: Arc::new(vec!["pgtrickle".to_string()]),
        }
    }

    pub fn new_with_config(
        catalog: Arc<Mutex<CatalogStore>>,
        auth: Arc<AuthConfig>,
        notify_manager: Arc<NotifyManager>,
        extension_schemas: Arc<Vec<String>>,
    ) -> Self {
        Self {
            catalog,
            session: Arc::new(Mutex::new(SessionState::new())),
            parser: Arc::new(SlateDuckQueryParser),
            auth,
            notify_manager,
            extension_schemas,
        }
    }
}

/// Constant-time byte slice equality comparison to resist timing attacks.
fn ct_bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Startup handler that enforces authentication when configured.
///
/// When `AuthConfig::is_enabled()` returns false, connections are accepted
/// without any credential check (noop). When it returns true, cleartext
/// password auth is required; username is verified before issuing the
/// password challenge, and the password is compared in constant time.
///
/// When `tls_required` is true and the client connects without TLS, the
/// connection is rejected immediately with a fatal error.
pub struct SlateDuckStartupHandler {
    auth: Arc<AuthConfig>,
    tls_required: bool,
}

impl SlateDuckStartupHandler {
    pub fn new(auth: Arc<AuthConfig>) -> Self {
        Self {
            auth,
            tls_required: false,
        }
    }

    pub fn new_with_tls_required(auth: Arc<AuthConfig>, tls_required: bool) -> Self {
        Self { auth, tls_required }
    }
}

#[async_trait]
impl pgwire::api::auth::StartupHandler for SlateDuckStartupHandler {
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
                if !self.auth.is_enabled() {
                    finish_authentication(client, &DefaultServerParameterProvider::default())
                        .await?;
                } else {
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
                    client
                        .send(PgWireBackendMessage::Authentication(
                            Authentication::CleartextPassword,
                        ))
                        .await?;
                }
            }
            PgWireFrontendMessage::PasswordMessageFamily(pwd) if self.auth.is_enabled() => {
                let pwd = pwd.into_password()?;
                let expected = self.auth.password.as_deref().unwrap_or("").as_bytes();
                if ct_bytes_eq(pwd.password.as_bytes(), expected) {
                    finish_authentication(client, &DefaultServerParameterProvider::default())
                        .await?;
                } else {
                    let error_info = ErrorInfo::new(
                        "FATAL".to_owned(),
                        "28P01".to_owned(),
                        "Password authentication failed".to_owned(),
                    );
                    client
                        .feed(PgWireBackendMessage::ErrorResponse(ErrorResponse::from(
                            error_info,
                        )))
                        .await?;
                    client.close().await?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[async_trait]
impl SimpleQueryHandler for SlateDuckHandler {
    async fn do_query<'a, 'b: 'a, C>(
        &'b self,
        _client: &mut C,
        query: &'a str,
    ) -> PgWireResult<Vec<Response<'a>>>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let params = ParamValues::default();
        let mut session = self.session.lock().await;
        match executor::execute_sql(
            query,
            &params,
            &self.catalog,
            &mut session,
            &self.notify_manager,
            &self.extension_schemas,
        )
        .await
        {
            Ok(responses) => Ok(responses),
            Err(e) => Err(e.into()),
        }
    }
}

/// Query parser that stores SQL strings.
#[derive(Debug, Clone)]
pub struct SlateDuckQueryParser;

#[async_trait]
impl QueryParser for SlateDuckQueryParser {
    type Statement = String;

    async fn parse_sql(&self, sql: &str, _types: &[Type]) -> PgWireResult<Self::Statement> {
        Ok(sql.to_owned())
    }
}

#[async_trait]
impl ExtendedQueryHandler for SlateDuckHandler {
    type Statement = String;
    type QueryParser = SlateDuckQueryParser;

    fn query_parser(&self) -> Arc<Self::QueryParser> {
        self.parser.clone()
    }

    async fn do_query<'a, 'b: 'a, C>(
        &'b self,
        _client: &mut C,
        portal: &'a Portal<Self::Statement>,
        _max_rows: usize,
    ) -> PgWireResult<Response<'a>>
    where
        C: ClientInfo + ClientPortalStore + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::PortalStore: PortalStore<Statement = Self::Statement>,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let sql = &portal.statement.statement;

        // Extract parameters from portal, handling binary-encoded integers.
        // tokio-postgres (and DuckDB) always send parameters in binary format;
        // for integer types we must decode the big-endian bytes to decimal
        // strings so that the string-based ParamValues can parse them.
        //
        // The stored `portal.statement.parameter_types` reflects what the client
        // declared in its Parse message (usually UNKNOWN because tokio-postgres
        // relies on DescribeStatement to learn types). We use `describe_params_for_sql`
        // as an authoritative fallback so binary INT8 bytes are always decoded correctly.
        let inferred_types = describe_params_for_sql(sql);
        let param_values: Vec<Option<String>> = portal
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| {
                p.as_ref().map(|b| {
                    if portal.parameter_format.is_binary(i) {
                        // Prefer a non-UNKNOWN stored type; fall back to inferred type.
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

        let mut session = self.session.lock().await;
        match executor::execute_sql(
            sql,
            &params,
            &self.catalog,
            &mut session,
            &self.notify_manager,
            &self.extension_schemas,
        )
        .await
        {
            Ok(mut responses) => {
                if let Some(resp) = responses.pop() {
                    Ok(resp)
                } else {
                    Ok(Response::EmptyQuery)
                }
            }
            Err(e) => Err(e.into()),
        }
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
        let fields = describe_fields_for_sql(sql);

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
        let fields = describe_fields_for_sql(sql);
        Ok(DescribePortalResponse::new(fields))
    }
}

/// Server handlers collection for SlateDuck.
pub struct SlateDuckServerHandlers {
    pub handler: Arc<SlateDuckHandler>,
    pub startup: Arc<SlateDuckStartupHandler>,
    pub copy_handler: Arc<NoopCopyHandler>,
    pub error_handler: Arc<NoopErrorHandler>,
}

impl SlateDuckServerHandlers {
    pub fn new(catalog: Arc<Mutex<CatalogStore>>) -> Self {
        let auth = Arc::new(AuthConfig::default());
        Self {
            handler: Arc::new(SlateDuckHandler::new(catalog)),
            startup: Arc::new(SlateDuckStartupHandler::new(auth)),
            copy_handler: Arc::new(NoopCopyHandler),
            error_handler: Arc::new(NoopErrorHandler),
        }
    }

    pub fn new_with_auth(catalog: Arc<Mutex<CatalogStore>>, auth: Arc<AuthConfig>) -> Self {
        Self {
            handler: Arc::new(SlateDuckHandler::new_with_auth(catalog, auth.clone())),
            startup: Arc::new(SlateDuckStartupHandler::new(auth)),
            copy_handler: Arc::new(NoopCopyHandler),
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
        Self {
            handler: Arc::new(SlateDuckHandler::new_with_config(
                catalog,
                auth.clone(),
                notify_manager,
                extension_schemas,
            )),
            startup: Arc::new(SlateDuckStartupHandler::new_with_tls_required(
                auth,
                tls_required,
            )),
            copy_handler: Arc::new(NoopCopyHandler),
            error_handler: Arc::new(NoopErrorHandler),
        }
    }
}

impl PgWireServerHandlers for SlateDuckServerHandlers {
    type StartupHandler = SlateDuckStartupHandler;
    type SimpleQueryHandler = SlateDuckHandler;
    type ExtendedQueryHandler = SlateDuckHandler;
    type CopyHandler = NoopCopyHandler;
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
    use slateduck_sql::StatementKind;
    let kind =
        slateduck_sql::classify_statement(sql).unwrap_or(StatementKind::Unsupported(String::new()));
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
fn describe_fields_for_sql(sql: &str) -> Vec<pgwire::api::results::FieldInfo> {
    use pgwire::api::results::{FieldFormat, FieldInfo};

    let kind = slateduck_sql::classify_statement(sql)
        .unwrap_or(slateduck_sql::StatementKind::Unsupported(String::new()));

    /// Quick helper to build a FieldInfo for text-type metadata columns.
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
        slateduck_sql::StatementKind::SelectVersion => vec![text_col!("version")],
        slateduck_sql::StatementKind::SelectCurrentSchema => vec![text_col!("current_schema")],
        slateduck_sql::StatementKind::SelectCurrentDatabase => {
            vec![text_col!("current_database")]
        }
        slateduck_sql::StatementKind::SelectPgType => vec![
            FieldInfo::new("oid".to_string(), None, None, Type::INT4, FieldFormat::Text),
            text_col!("typname"),
        ],
        slateduck_sql::StatementKind::SelectMaxSnapshot
        | slateduck_sql::StatementKind::SelectMaxSnapshotAfter => {
            vec![int8_col!("max")]
        }
        slateduck_sql::StatementKind::ShowVariable(ref var) => {
            vec![text_col!(var.as_str())]
        }
        // Catalog table schemas — must match the executor's make_*_response column lists.
        slateduck_sql::StatementKind::SelectSchemas => vec![
            int8_col!("schema_id"),
            int8_col!("begin_snapshot"),
            int8_col!("end_snapshot"),
            text_col!("schema_uuid"),
            text_col!("schema_name"),
            text_col!("path"),
            FieldInfo::new(
                "path_is_relative".to_string(),
                None,
                None,
                Type::BOOL,
                FieldFormat::Text,
            ),
        ],
        slateduck_sql::StatementKind::SelectTables => vec![
            int8_col!("table_id"),
            int8_col!("begin_snapshot"),
            int8_col!("end_snapshot"),
            int8_col!("schema_id"),
            text_col!("table_uuid"),
            text_col!("table_name"),
            text_col!("data_path"),
        ],
        slateduck_sql::StatementKind::SelectDataFiles => vec![
            int8_col!("data_file_id"),
            int8_col!("table_id"),
            int8_col!("begin_snapshot"),
            int8_col!("end_snapshot"),
            int8_col!("file_order"),
            text_col!("path"),
            FieldInfo::new(
                "path_is_relative".to_string(),
                None,
                None,
                Type::BOOL,
                FieldFormat::Text,
            ),
            text_col!("file_format"),
        ],
        _ => vec![],
    }
}
