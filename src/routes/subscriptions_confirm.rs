use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use hyper::StatusCode;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{error, field::debug, Span};
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
    #[error("There is no subscriber associated with the provided token")]
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

#[tracing::instrument(skip_all, fields(token=params.subscription_token, uuid))]
pub async fn confirm(
    state: State<AppState>,
    params: Query<Params>,
) -> Result<(), ConfirmationError> {
    let subscriber_id = get_subscriber_id(&state.db_pool, &params.subscription_token).await?;
    Span::current().record("uuid", debug(subscriber_id));

    confirm_subscriber(&state.db_pool, subscriber_id).await?;

    Ok(())
}

#[tracing::instrument(skip_all)]
async fn get_subscriber_id(
    db_pool: &PgPool,
    subscription_token: &str,
) -> Result<Uuid, ConfirmationError> {
    let result = sqlx::query!(
        r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1"#,
        subscription_token
    )
    .fetch_optional(db_pool)
    .await
    .wrap_and_log_err("Failed to retrieve subscriber id with the provided token")?;

    let subscriber_id = result
        .map(|r| r.subscriber_id)
        .ok_or(ConfirmationError::UnknownToken)
        .log_err()?;

    Ok(subscriber_id)
}

#[tracing::instrument(skip_all)]
async fn confirm_subscriber(
    db_pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<(), ConfirmationError> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id
    )
    .execute(db_pool)
    .await
    .wrap_and_log_err("Failed to update the subscriber status to `confirmed`")?;

    Ok(())
}
