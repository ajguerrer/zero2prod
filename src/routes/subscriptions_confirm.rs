use axum::extract::{Query, State};
use hyper::StatusCode;
use serde::Deserialize;
use sqlx::{query, PgPool};
use tracing::{error, info};
use uuid::Uuid;

use crate::startup::AppState;

#[derive(Deserialize)]
pub struct Params {
    subscription_token: String,
}

#[tracing::instrument(skip_all, fields(params.subscription_token))]
pub async fn confirm(
    state: State<AppState>,
    params: Query<Params>,
) -> Result<(), (StatusCode, String)> {
    let subscriber_id = get_subscriber_id_from_token(&state.db_pool, &params.subscription_token)
        .await
        .map_err(|err| {
            let msg = "failed to find subscriber by subscription token".to_string();
            error!(%err, msg);
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, String::new()))?;

    confirm_subscriber(&state.db_pool, subscriber_id)
        .await
        .map_err(|err| {
            let msg = "failed to confirm subscriber".to_string();
            error!(%err, msg);
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

    info!("subscription confirmed!");
    Ok(())
}

async fn get_subscriber_id_from_token(
    db_pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = query!(
        r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1"#,
        subscription_token
    )
    .fetch_optional(db_pool)
    .await?;

    Ok(result.map(|r| r.subscriber_id))
}

async fn confirm_subscriber(db_pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id
    )
    .execute(db_pool)
    .await?;

    Ok(())
}
