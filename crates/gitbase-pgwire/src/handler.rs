use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream;
use pgwire::api::auth::cleartext::CleartextPasswordAuthStartupHandler;
use pgwire::api::auth::{
    AuthSource, DefaultServerParameterProvider, LoginInfo, Password, StartupHandler,
};
use pgwire::api::query::SimpleQueryHandler;
use pgwire::api::results::{DataRowEncoder, FieldFormat, FieldInfo, QueryResponse, Response, Tag};
use pgwire::api::{ClientInfo, PgWireServerHandlers, Type};
use pgwire::error::{PgWireError, PgWireResult};
use sqlx::postgres::PgRow;
use sqlx::{Column, PgPool, Row, TypeInfo};
use std::path::PathBuf;

use gitbase_loader::{hydrate_blobs, BlobHydrationConfig};

/// Handles incoming PostgreSQL wire-protocol queries by forwarding them to a
/// real PostgreSQL database through sqlx.
pub struct GitbaseHandler {
    pool: PgPool,
    repo_roots: Vec<PathBuf>,
    blob_config: BlobHydrationConfig,
}

impl GitbaseHandler {
    pub fn new(pool: PgPool, repo_roots: Vec<PathBuf>, blob_config: BlobHydrationConfig) -> Self {
        Self {
            pool,
            repo_roots,
            blob_config,
        }
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
            self.maybe_hydrate_blobs(query).await?;
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

impl GitbaseHandler {
    async fn maybe_hydrate_blobs(&self, query: &str) -> PgWireResult<()> {
        let blob_hashes = extract_blob_hashes(query);
        if blob_hashes.is_empty() {
            return Ok(());
        }

        hydrate_blobs(
            &self.pool,
            &self.repo_roots,
            &blob_hashes,
            &self.blob_config,
        )
        .await
        .map_err(|e| {
            PgWireError::ApiError(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        Ok(())
    }
}

fn extract_blob_hashes(query: &str) -> Vec<String> {
    let lower = query.to_lowercase();
    let mut hashes = Vec::new();
    let mut offset = 0;

    while let Some(pos) = lower[offset..].find("blob_hash") {
        let start = offset + pos;
        let fragment = &query[start..];
        if let Some(quote_start) = fragment.find('\'') {
            let start_idx = start + quote_start + 1;
            if query.len() >= start_idx + 40 {
                let candidate = &query[start_idx..start_idx + 40];
                if candidate.chars().all(|c| c.is_ascii_hexdigit()) {
                    hashes.push(candidate.to_string());
                }
            }
        }
        offset = start + 9;
    }

    hashes.sort();
    hashes.dedup();
    if hashes.len() > 50 {
        hashes.truncate(50);
    }
    hashes
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
    startup_handler:
        Arc<CleartextPasswordAuthStartupHandler<GitbaseAuthSource, DefaultServerParameterProvider>>,
}

impl GitbaseServerFactory {
    pub fn new(
        pool: PgPool,
        repo_roots: Vec<PathBuf>,
        blob_config: BlobHydrationConfig,
        auth_user: String,
        auth_password: String,
    ) -> Self {
        let auth_source = GitbaseAuthSource {
            user: auth_user,
            password: auth_password,
        };
        let startup_handler = Arc::new(CleartextPasswordAuthStartupHandler::new(
            auth_source,
            DefaultServerParameterProvider::default(),
        ));
        Self {
            handler: Arc::new(GitbaseHandler::new(pool, repo_roots, blob_config)),
            startup_handler,
        }
    }
}

impl PgWireServerHandlers for GitbaseServerFactory {
    fn simple_query_handler(&self) -> Arc<impl SimpleQueryHandler> {
        self.handler.clone()
    }

    fn startup_handler(&self) -> Arc<impl StartupHandler> {
        self.startup_handler.clone()
    }
}

#[derive(Debug, Clone)]
struct GitbaseAuthSource {
    user: String,
    password: String,
}

#[async_trait]
impl AuthSource for GitbaseAuthSource {
    async fn get_password(&self, login: &LoginInfo) -> PgWireResult<Password> {
        if login.user() == Some(self.user.as_str()) {
            Ok(Password::new(None, self.password.as_bytes().to_vec()))
        } else {
            Err(PgWireError::InvalidPassword(
                login.user().unwrap_or_default().to_string(),
            ))
        }
    }
}
