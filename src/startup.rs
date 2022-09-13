use axum::{
    routing::{get, post, IntoMakeService},
    Router, Server,
};
use hyper::server::conn::AddrIncoming;
use sqlx::PgPool;

use crate::routes::{health_check, subscribe};

pub fn run(
    address: &str,
    db_pool: PgPool,
) -> Server<AddrIncoming, IntoMakeService<Router<PgPool>>> {
    let app = Router::with_state(db_pool)
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe));
    Server::bind(&address.parse().unwrap()).serve(app.into_make_service())
}
