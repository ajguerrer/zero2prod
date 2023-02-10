use anyhow::{anyhow, Result};
use axum::{
    headers::HeaderName,
    http::HeaderValue,
    response::{IntoResponse, Response},
};
use hyper::{body::to_bytes, HeaderMap, StatusCode};
use sqlx::{postgres::PgHasArrayType, query, query_unchecked, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::IdempotencyKey;

pub enum NextAction {
    StartProcessing(Transaction<'static, Postgres>),
    ReturnSavedResponse(Response),
}

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

impl PgHasArrayType for HeaderPairRecord {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_header_pair")
    }
}

pub async fn try_processing(
    db_pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<NextAction> {
    let mut transaction = db_pool.begin().await?;
    if query!(
        r#"
    INSERT INTO idempotency (
        user_id,
        idempotency_key,
        created_at
    )
    VALUES ($1, $2, now())
    ON CONFLICT DO NOTHING
    "#,
        user_id,
        idempotency_key.as_ref()
    )
    .execute(&mut transaction)
    .await?
    .rows_affected()
        > 0
    {
        Ok(NextAction::StartProcessing(transaction))
    } else {
        let saved_response = get_saved_response(db_pool, idempotency_key, user_id)
            .await?
            .ok_or_else(|| anyhow!("Expected a saved response, but didn't find one"))?;
        Ok(NextAction::ReturnSavedResponse(saved_response))
    }
}

pub async fn get_saved_response(
    db_pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<Option<Response>> {
    match query!(
        r#"
    SELECT
        response_status_code as "response_status_code!",
        response_headers as "response_headers!: Vec<HeaderPairRecord>",
        response_body as "response_body!"
    FROM idempotency
    WHERE
        user_id = $1 AND
        idempotency_key = $2
    "#,
        user_id,
        idempotency_key.as_ref()
    )
    .fetch_optional(db_pool)
    .await?
    {
        Some(r) => {
            let mut headers = HeaderMap::new();
            for HeaderPairRecord { name, value } in r.response_headers {
                headers.append(HeaderName::try_from(name)?, HeaderValue::try_from(value)?);
            }
            let status = StatusCode::from_u16(r.response_status_code as u16)?;
            Ok(Some((status, headers, r.response_body).into_response()))
        }
        None => Ok(None),
    }
}

pub async fn save_response(
    mut transaction: Transaction<'static, Postgres>,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
    response: Response,
) -> Result<Response> {
    let status_code = response.status().as_u16() as i16;
    let (head, body) = response.into_parts();
    let body = to_bytes(body).await?;
    let headers: Vec<_> = head
        .headers
        .iter()
        .map(|(k, v)| HeaderPairRecord {
            name: k.to_string(),
            value: v.as_bytes().to_owned(),
        })
        .collect();

    query_unchecked!(
        r#"
        UPDATE idempotency
        SET 
            response_status_code = $3,
            response_headers = $4,
            response_body = $5
        WHERE
            user_id = $1 AND
            idempotency_key = $2
    "#,
        user_id,
        idempotency_key.as_ref(),
        status_code,
        headers,
        body.as_ref()
    )
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;

    Ok((head, body).into_response())
}
