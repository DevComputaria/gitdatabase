use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream;
use pgwire::api::auth::noop::NoopStartupHandler;
use pgwire::api::query::SimpleQueryHandler;
use pgwire::api::results::{DataRowEncoder, FieldFormat, FieldInfo, QueryResponse, Response, Tag};
use pgwire::api::{ClientInfo, PgWireServerHandlers, Type};
use pgwire::error::{PgWireError, PgWireResult};
use pgwire::messages::{PgWireBackendMessage, PgWireFrontendMessage};
use sqlx::postgres::PgRow;
use sqlx::{Column, PgPool, Row, TypeInfo};

use futures::Sink;

/// Handles incoming PostgreSQL wire-protocol queries by forwarding them to a
/// real PostgreSQL database through sqlx.
pub struct GitbaseHandler {
    pool: PgPool,
}

impl GitbaseHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NoopStartupHandler for GitbaseHandler {
    async fn post_startup<C>(
        &self,
        client: &mut C,
        _message: PgWireFrontendMessage,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        tracing::info!(
            addr = %client.socket_addr(),
            "client connected"
        );
        Ok(())
    }
}

#[async_trait]
impl SimpleQueryHandler for GitbaseHandler {
    async fn do_query<C>(&self, _client: &mut C, query: &str) -> PgWireResult<Vec<Response>>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        tracing::debug!(query, "forwarding query to PostgreSQL");

        let trimmed = query.trim().to_uppercase();

        // For SELECT queries, fetch rows and stream them back.
        if trimmed.starts_with("SELECT") {
            let rows: Vec<PgRow> = sqlx::query(query)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PgWireError::ApiError(Box::new(e)))?;

            if rows.is_empty() {
                let schema = Arc::new(vec![]);
                let data_row_stream = stream::empty();
                return Ok(vec![Response::Query(QueryResponse::new(
                    schema,
                    data_row_stream,
                ))]);
            }

            // Build schema from the first row's columns.
            let columns = rows[0].columns();
            let field_infos: Vec<FieldInfo> = columns
                .iter()
                .map(|col| {
                    let pg_type = sqlx_type_to_pgwire(col.type_info().name());
                    FieldInfo::new(
                        col.name().to_string(),
                        None,
                        None,
                        pg_type,
                        FieldFormat::Text,
                    )
                })
                .collect();
            let schema = Arc::new(field_infos);

            // Encode all rows.
            let schema_ref = schema.clone();
            let encoded_rows: Vec<_> = rows
                .iter()
                .map(|row| {
                    let mut encoder = DataRowEncoder::new(schema_ref.clone());
                    for (i, col) in row.columns().iter().enumerate() {
                        let value: Option<String> = row.try_get(i).unwrap_or_else(|_| {
                            // Fall back: try to get raw bytes and convert
                            match col.type_info().name() {
                                "INT4" | "INT8" | "INT2" => {
                                    row.try_get::<i64, _>(i).ok().map(|v| v.to_string())
                                }
                                "FLOAT4" | "FLOAT8" => {
                                    row.try_get::<f64, _>(i).ok().map(|v| v.to_string())
                                }
                                "BOOL" => row.try_get::<bool, _>(i).ok().map(|v| v.to_string()),
                                _ => row.try_get::<String, _>(i).ok(),
                            }
                        });
                        encoder.encode_field(&value).unwrap();
                    }
                    Ok(encoder.take_row())
                })
                .collect();

            let data_row_stream = stream::iter(encoded_rows);
            Ok(vec![Response::Query(QueryResponse::new(
                schema,
                data_row_stream,
            ))])
        } else {
            // Non-SELECT: execute and return affected rows.
            let result = sqlx::query(query)
                .execute(&self.pool)
                .await
                .map_err(|e| PgWireError::ApiError(Box::new(e)))?;
            Ok(vec![Response::Execution(
                Tag::new("OK").with_rows(result.rows_affected() as usize),
            )])
        }
    }
}

/// Maps sqlx type names to pgwire Type constants.
fn sqlx_type_to_pgwire(type_name: &str) -> Type {
    match type_name {
        "INT2" => Type::INT2,
        "INT4" => Type::INT4,
        "INT8" => Type::INT8,
        "FLOAT4" => Type::FLOAT4,
        "FLOAT8" => Type::FLOAT8,
        "BOOL" => Type::BOOL,
        "TEXT" => Type::TEXT,
        "VARCHAR" => Type::VARCHAR,
        "TIMESTAMP" => Type::TIMESTAMP,
        "TIMESTAMPTZ" => Type::TIMESTAMPTZ,
        "DATE" => Type::DATE,
        "UUID" => Type::UUID,
        "JSONB" | "JSON" => Type::JSONB,
        _ => Type::TEXT,
    }
}

/// Factory used by pgwire to obtain handler instances per connection.
pub struct GitbaseServerFactory {
    handler: Arc<GitbaseHandler>,
}

impl GitbaseServerFactory {
    pub fn new(pool: PgPool) -> Self {
        Self {
            handler: Arc::new(GitbaseHandler::new(pool)),
        }
    }
}

impl PgWireServerHandlers for GitbaseServerFactory {
    fn simple_query_handler(&self) -> Arc<impl SimpleQueryHandler> {
        self.handler.clone()
    }

    fn startup_handler(&self) -> Arc<impl pgwire::api::auth::StartupHandler> {
        self.handler.clone()
    }
}
