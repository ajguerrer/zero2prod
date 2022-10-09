use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use hyper::StatusCode;
use serde::Deserialize;
use sqlx::{query, PgPool};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    log::{LogErr, WrapAndLogErr},
    startup::AppState,
};

#[derive(Deserialize)]
pub struct Params {
    subscription_token: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ConfirmationError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
    #[error("there is no subscriber associated with the provided token")]
    UnknownToken,
}

impl IntoResponse for ConfirmationError {
    fn into_response(self) -> Response {
        let status = match self {
            ConfirmationError::UnknownToken => StatusCode::UNAUTHORIZED,
            ConfirmationError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}

#[tracing::instrument(skip_all, fields(token=params.subscription_token))]
pub async fn confirm(
    state: State<AppState>,
    params: Query<Params>,
) -> Result<(), ConfirmationError> {
    let subscriber_id = get_subscriber_id_from_token(&state.db_pool, &params.subscription_token)
        .await
        .wrap_and_log_err("failed to retrieve subscriber id with the provided token")?
        .ok_or(ConfirmationError::UnknownToken)
        .log_err()?;

    confirm_subscriber(&state.db_pool, subscriber_id)
        .await
        .wrap_and_log_err("failed to update the subscriber status to `confirmed`")?;

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
