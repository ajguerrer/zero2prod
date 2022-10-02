use std::iter;

use axum::{extract::State, Form};
use hyper::StatusCode;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Deserialize;
use sqlx::{query, types::chrono::Utc, Postgres, Transaction};
use tracing::{error, field::debug, info, instrument, Span};
use url::Url;
use uuid::Uuid;

use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::AppState,
};

#[derive(Debug, Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(data: FormData) -> Result<Self, Self::Error> {
        let email = SubscriberEmail::parse(data.email)?;
        let name = SubscriberName::parse(data.name)?;
        Ok(NewSubscriber { email, name })
    }
}

#[instrument(skip_all, fields(name=data.name, email=data.email, subscriber_id))]
pub async fn subscribe(
    state: State<AppState>,
    data: Form<FormData>,
) -> Result<(), (StatusCode, String)> {
    let subscriber_id = Uuid::new_v4();
    Span::current().record("subscriber_id", debug(subscriber_id));

    let new_subscriber =
        NewSubscriber::try_from(data.0).map_err(|err| (StatusCode::UNPROCESSABLE_ENTITY, err))?;

    let mut transaction = state.db_pool.begin().await.map_err(|err| {
        let msg = "failed to begin transaction".to_string();
        error!(%err, msg);
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?;

    insert_subscriber(&mut transaction, &subscriber_id, &new_subscriber)
        .await
        .map_err(|err| {
            let msg = "failed to execute query".to_string();
            error!(%err, msg);
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

    let subscription_token = generate_subscription_token();
    store_token(&mut transaction, &subscriber_id, &subscription_token)
        .await
        .map_err(|err| {
            let msg = "failed to store subscription token".to_string();
            error!(%err, msg=&msg);
            (StatusCode::INTERNAL_SERVER_ERROR, msg)
        })?;

    transaction.commit().await.map_err(|err| {
        let msg = "failed to commit transaction".to_string();
        error!(%err, msg);
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?;

    send_confirmation_email(
        &state.email_client,
        &new_subscriber,
        &state.base_url,
        &subscription_token,
    )
    .await
    .map_err(|err| {
        let msg = "failed to send confirmation email".to_string();
        error!(%err, msg);
        (StatusCode::INTERNAL_SERVER_ERROR, msg)
    })?;

    info!("added subscriber");
    Ok(())
}

async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: &Uuid,
    new_subscriber: &NewSubscriber,
) -> Result<(), sqlx::Error> {
    query!(
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

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: &Uuid,
    subscription_token: &str,
) -> Result<(), sqlx::Error> {
    query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    )
    .execute(transaction)
    .await?;

    Ok(())
}

async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: &NewSubscriber,
    base_url: &Url,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
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
}
