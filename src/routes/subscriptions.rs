use axum::{extract::State, http::StatusCode, Form};
use chrono::Utc;
use serde::Deserialize;
use sqlx::{query, PgPool};
use tracing::{error, field::debug, info, instrument, Span};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[instrument(skip_all, fields(name=data.name, email=data.email, request_id))]
pub async fn subscribe(
    db_pool: State<PgPool>,
    data: Form<FormData>,
) -> Result<(), (StatusCode, String)> {
    let request_id = Uuid::new_v4();
    Span::current().record("request_id", debug(request_id));

    match query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        request_id,
        data.email,
        data.name,
        Utc::now()
    )
    .execute(&*db_pool)
    .await
    {
        Ok(_) => {
            info!("added subscriber");
            Ok(())
        }
        Err(err) => {
            error!(%err, "failed to execute query");
            Err((StatusCode::INTERNAL_SERVER_ERROR, String::new()))
        }
    }
}
