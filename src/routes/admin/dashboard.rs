use axum::response::{IntoResponse, Response};
use axum::{extract::State, response::Html};
use hyper::StatusCode;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::log::WrapAndLogErr;
use crate::startup::AppState;
use crate::user_session::UserId;

#[derive(thiserror::Error, Debug)]
#[error("Something went wrong")]
pub struct DashboardError(#[from] anyhow::Error);

impl IntoResponse for DashboardError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

#[instrument(skip_all, fields(uuid=?*user_id))]
pub async fn admin_dashboard(
    state: State<AppState>,
    user_id: UserId,
) -> Result<Html<String>, DashboardError> {
    let username = get_username(*user_id, &state.db_pool)
        .await
        .wrap_and_log_err("Failed to query username")?;

    Ok(Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Admin dashboard</title>
</head>
<body>
    <p>Welcome {username}!</p>
    <p>Available actions:</p>
    <ol>
        <li>
            <form name="sendNewsletterForm" action="/admin/newsletters" method="get">
                <input type="submit" value="Send a Newsletter">
            </form>    
        </li>
        <li>
            <form name="changePasswordForm" action="/admin/password" method="get">
                <input type="submit" value="Change password">
            </form>    
        </li>
        <li>
            <form name="logoutForm" action="/admin/logout" method="post">
                <input type="submit" value="Logout">
            </form>
        </li>
    </ol>
</body>
</html>"#
    )))
}

pub async fn get_username(user_id: Uuid, db_pool: &PgPool) -> Result<String, sqlx::Error> {
    sqlx::query!(
        r#"SELECT username
        FROM users
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_one(db_pool)
    .await
    .map(|row| row.username)
}
