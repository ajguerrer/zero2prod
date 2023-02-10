use std::iter;

use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Form,
};
use hyper::StatusCode;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Deserialize;
use sqlx::{types::chrono::Utc, PgPool, Postgres, Transaction};
use tracing::{error, field::debug, instrument, Span};
use url::Url;
use uuid::Uuid;

use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    log::{LogErr, WrapAndLogErr},
    startup::AppState,
};

#[derive(Debug, Deserialize)]
pub struct NewSubscriberForm {
    email: String,
    name: String,
}

impl TryFrom<NewSubscriberForm> for NewSubscriber {
    type Error = String;

    fn try_from(data: NewSubscriberForm) -> Result<Self, Self::Error> {
        let email = SubscriberEmail::parse(data.email)?;
        let name = SubscriberName::parse(data.name)?;
        Ok(NewSubscriber { email, name })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl IntoResponse for SubscribeError {
    fn into_response(self) -> Response {
        let status = match self {
            SubscribeError::ValidationError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}

#[instrument(skip_all, fields(name=form.name, email=form.email, uuid))]
pub async fn subscribe(
    state: State<AppState>,
    form: Form<NewSubscriberForm>,
) -> Result<(), SubscribeError> {
    let subscriber_id = Uuid::new_v4();
    Span::current().record("uuid", debug(subscriber_id));

    let new_subscriber = NewSubscriber::try_from(form.0)
        .map_err(SubscribeError::ValidationError)
        .log_err()?;
    let subscription_token =
        add_subscriber(&state.db_pool, &new_subscriber, &subscriber_id).await?;

    send_confirmation_email(
        &state.email_client,
        &new_subscriber,
        &state.base_url,
        &subscription_token,
    )
    .await?;

    Ok(())
}

#[instrument(skip_all)]
async fn add_subscriber(
    db_pool: &PgPool,
    new_subscriber: &NewSubscriber,
    subscriber_id: &Uuid,
) -> Result<String, SubscribeError> {
    let mut transaction = db_pool
        .begin()
        .await
        .wrap_and_log_err("Failed to begin transaction")?;

    store_subscriber(&mut transaction, subscriber_id, new_subscriber)
        .await
        .wrap_and_log_err("Failed to execute query")?;

    let subscription_token = generate_subscription_token();
    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .wrap_and_log_err("Failed to store subscription token")?;

    transaction
        .commit()
        .await
        .wrap_and_log_err("Failed to commit transaction")?;

    Ok(subscription_token)
}

#[instrument(skip_all)]
async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: &NewSubscriber,
    base_url: &Url,
    subscription_token: &str,
) -> Result<(), SubscribeError> {
    let confirmation_link = base_url
        .join(&format!(
            "subscriptions/confirm?subscription_token={subscription_token}",
        ))
        .unwrap();
    let html_body = format!(
        "Welcome to out newsletter!<br />Click <a href=\"{confirmation_link}\">here</a> to confirm your subscription.",
    );
    let text_body = format!(
        "Welcome to out newsletter!\nVisit {confirmation_link} to confirm your subscription.",
    );
    email_client
        .send(&new_subscriber.email, "Welcome!", &html_body, &text_body)
        .await
        .wrap_and_log_err("Failed to send confirmation email")?;
    Ok(())
}

async fn store_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: &Uuid,
    new_subscriber: &NewSubscriber,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(transaction)
    .await?;

    Ok(())
}

pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: &Uuid,
    subscription_token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    )
    .execute(transaction)
    .await?;

    Ok(())
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}
