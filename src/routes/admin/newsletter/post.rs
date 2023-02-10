use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_flash::Flash;
use hyper::StatusCode;
use serde::Deserialize;
use sqlx::{query, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    idempotency::{save_response, try_processing, IdempotencyKey, NextAction},
    log::WrapAndLogErr,
    startup::AppState,
    user_session::UserId,
};

#[derive(Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct PublishError(#[from] anyhow::Error);

impl IntoResponse for PublishError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

#[tracing::instrument(skip_all, fields(uuid=?*user_id))]
pub async fn publish_newsletter(
    state: State<AppState>,
    user_id: UserId,
    flash: Flash,
    form: Form<FormData>,
) -> Result<Response, PublishError> {
    let idempotency_key = IdempotencyKey::try_from(form.idempotency_key.clone())?;
    let mut transaction = match try_processing(&state.db_pool, &idempotency_key, *user_id)
        .await
        .wrap_and_log_err(format!(
            "Failed to retrieve saved response with key {}",
            &idempotency_key.as_ref()
        ))? {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => return Ok(saved_response),
    };
    let issue_id = insert_newsletter_issue(&mut transaction, &form)
        .await
        .wrap_and_log_err("Failed to store newsletter issue")?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .wrap_and_log_err("Failed to enqueue newsletter delivery")?;

    let response = (
        flash.success("The newsletter issue has been accepted - emails will go out shortly."),
        Redirect::to("/admin/newsletters"),
    )
        .into_response();
    let response = save_response(transaction, &idempotency_key, *user_id, response)
        .await
        .wrap_and_log_err(format!(
            "Failed to save response with key {}",
            &idempotency_key.as_ref(),
        ))?;

    Ok(response)
}

#[tracing::instrument(skip_all)]
pub async fn insert_newsletter_issue(
    transaction: &mut Transaction<'static, Postgres>,
    form: &FormData,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    query!(
        r#"
    INSERT INTO newsletter_issues (
        newsletter_issue_id,
        title,
        text_content,
        html_content,
        published_at
    )
    VALUES ($1, $2, $3, $4, now())
    "#,
        newsletter_issue_id,
        form.title,
        form.text_content,
        form.html_content
    )
    .execute(transaction)
    .await?;

    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'static, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    query!(
        r#"
    INSERT INTO issue_delivery_queue (
        newsletter_issue_id,
        subscriber_email,
        n_retries,
        execute_after
    )
    SELECT $1, email, 0, now()
    FROM subscriptions
    WHERE status = 'confirmed'
    "#,
        newsletter_issue_id
    )
    .execute(transaction)
    .await?;

    Ok(())
}
