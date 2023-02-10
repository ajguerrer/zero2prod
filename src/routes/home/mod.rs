use axum::response::Html;
use tracing::instrument;

#[instrument]
pub async fn home() -> Html<&'static str> {
    Html(include_str!("home.html"))
}
