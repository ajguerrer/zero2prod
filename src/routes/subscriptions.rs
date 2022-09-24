use axum::{extract::State, Form};
use hyper::StatusCode;
use serde::Deserialize;
use sqlx::{query, types::chrono::Utc, PgPool};
use tracing::{error, field::debug, info, instrument, Span};
use uuid::Uuid;

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};

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

#[instrument(skip_all, fields(name=data.name, email=data.email, request_id))]
pub async fn subscribe(
    db_pool: State<PgPool>,
    data: Form<FormData>,
) -> Result<(), (StatusCode, String)> {
    let request_id = Uuid::new_v4();
    Span::current().record("request_id", debug(request_id));

    let new_subscriber =
        NewSubscriber::try_from(data.0).map_err(|err| (StatusCode::UNPROCESSABLE_ENTITY, err))?;

    match query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        request_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(&*db_pool)
    .await
    {
        Err(err) => {
            error!(%err, "failed to execute query");
            Err((StatusCode::INTERNAL_SERVER_ERROR, String::new()))
        }
        _ => {
            info!("added subscriber");
            Ok(())
        }
    }
}
