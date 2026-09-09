//! Shared bounded pipeline for DuckLake metadata responses.

use std::sync::Arc;

use futures::stream::{BoxStream, Stream, StreamExt};
use pgwire::api::results::{DataRowEncoder, FieldInfo, QueryResponse, Response};
use pgwire::error::{ErrorInfo, PgWireError, PgWireResult};
use pgwire::messages::data::DataRow;
use rocklake_catalog::error::CatalogResult;

use crate::error::RockLakeError;
use crate::lifecycle::OperationClass;

/// A value supplied to the one metadata row encoder.
#[derive(Debug)]
pub enum MetadataValue {
    Null,
    Text(Option<String>),
    Int8(Option<i64>),
    Bool(Option<bool>),
    Bytes(Option<Vec<u8>>),
}

impl MetadataValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(Some(value.into()))
    }

    pub fn optional_text(value: Option<String>) -> Self {
        Self::Text(value)
    }

    pub fn int8(value: u64) -> Self {
        Self::Int8(Some(value as i64))
    }

    pub fn optional_int8(value: Option<u64>) -> Self {
        Self::Int8(value.map(|value| value as i64))
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool(Some(value))
    }

    pub fn optional_bool(value: Option<bool>) -> Self {
        Self::Bool(value)
    }
}

/// Encode one row from the schema registry. Arity errors are returned through
/// the stream so a response can never turn a partial row into success.
pub fn encode_metadata_row(
    schema: Arc<Vec<FieldInfo>>,
    values: Vec<MetadataValue>,
) -> PgWireResult<DataRow> {
    if values.len() != schema.len() {
        return Err(PgWireError::UserError(Box::new(ErrorInfo::new(
            "ERROR".to_string(),
            "XX000".to_string(),
            format!(
                "metadata row has {} fields, expected {}",
                values.len(),
                schema.len()
            ),
        ))));
    }

    let mut encoder = DataRowEncoder::new(schema.clone());
    for (field, value) in schema.iter().zip(values) {
        let datatype = field.datatype();
        let format = field.format();
        match value {
            MetadataValue::Null => {
                encoder.encode_field_with_type_and_format(
                    &Option::<String>::None,
                    datatype,
                    format,
                )?;
            }
            MetadataValue::Text(value) => {
                encoder.encode_field_with_type_and_format(&value, datatype, format)?;
            }
            MetadataValue::Int8(value) => {
                encoder.encode_field_with_type_and_format(&value, datatype, format)?;
            }
            MetadataValue::Bool(value) => {
                encoder.encode_field_with_type_and_format(&value, datatype, format)?;
            }
            MetadataValue::Bytes(value) => {
                encoder.encode_field_with_type_and_format(&value, datatype, format)?;
            }
        }
    }
    encoder.finish()
}

/// One metadata relation flowing through the common response boundary.
pub struct CatalogRowStream {
    pub snapshot_id: u64,
    pub schema: Arc<Vec<FieldInfo>>,
    pub operation_class: OperationClass,
    pub ordering: &'static str,
    pub row_estimate: Option<usize>,
    rows: BoxStream<'static, PgWireResult<DataRow>>,
}

impl CatalogRowStream {
    /// Adapt a fallible catalog stream without collecting it.
    pub fn from_catalog_rows<T, F>(
        snapshot_id: u64,
        schema: Arc<Vec<FieldInfo>>,
        operation_class: OperationClass,
        ordering: &'static str,
        row_estimate: Option<usize>,
        rows: BoxStream<'static, CatalogResult<T>>,
        encode: F,
    ) -> Self
    where
        T: Send + 'static,
        F: Fn(T, Arc<Vec<FieldInfo>>) -> PgWireResult<DataRow> + Send + Sync + 'static,
    {
        let schema_for_rows = schema.clone();
        let rows = rows
            .map(move |row| match row {
                Ok(row) => encode(row, schema_for_rows.clone()),
                Err(error) => Err(RockLakeError::from(error).into()),
            })
            .boxed();
        Self {
            snapshot_id,
            schema,
            operation_class,
            ordering,
            row_estimate,
            rows,
        }
    }

    /// Adapt already encoded rows for compatibility builders.
    pub fn from_encoded_rows<S>(
        snapshot_id: u64,
        schema: Arc<Vec<FieldInfo>>,
        operation_class: OperationClass,
        ordering: &'static str,
        row_estimate: Option<usize>,
        rows: S,
    ) -> Self
    where
        S: Stream<Item = PgWireResult<DataRow>> + Send + Unpin + 'static,
    {
        Self {
            snapshot_id,
            schema,
            operation_class,
            ordering,
            row_estimate,
            rows: rows.boxed(),
        }
    }

    /// Bound a compatibility stream before it reaches the wire.
    pub fn take(mut self, max_rows: usize) -> Self {
        self.rows = self.rows.take(max_rows).boxed();
        self
    }

    pub fn into_response(self, command_tag: &str) -> Response<'static> {
        let mut response = QueryResponse::new(self.schema, self.rows);
        response.set_command_tag(command_tag);
        Response::Query(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use pgwire::api::results::FieldFormat;
    use pgwire::api::Type;

    #[tokio::test]
    async fn schema_driven_encoder_rejects_wrong_arity() {
        let schema = Arc::new(vec![FieldInfo::new(
            "id".to_string(),
            None,
            None,
            Type::INT8,
            FieldFormat::Text,
        )]);
        let row = encode_metadata_row(schema, vec![]);
        assert!(row.is_err());
    }

    #[tokio::test]
    async fn stream_keeps_catalog_errors_in_the_response() {
        let schema = Arc::new(vec![]);
        let rows = stream::iter(vec![Err::<(), _>(
            rocklake_catalog::error::CatalogError::InvalidInput("broken".to_string()),
        )])
        .boxed();
        let stream = CatalogRowStream::from_catalog_rows(
            7,
            schema,
            OperationClass::InteractiveScan,
            "id ASC",
            None,
            rows,
            |_, schema| encode_metadata_row(schema, vec![]),
        );
        let Response::Query(response) = stream.into_response("SELECT") else {
            panic!("expected query response");
        };
        let mut rows = response.data_rows();
        assert!(rows.next().await.unwrap().is_err());
    }
}
