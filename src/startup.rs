use std::{net::SocketAddr, time::Duration};

use axum::{
    routing::{get, post, IntoMakeService},
    Router, Server,
};
use hyper::{server::conn::AddrIncoming, Error};
use sqlx::{postgres::PgPoolOptions, PgPool};

use url::Url;

use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{confirm, health_check, subscribe},
};

pub struct App {
    server: Server<AddrIncoming, IntoMakeService<Router<AppState>>>,
}

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub email_client: EmailClient,
    pub base_url: Url,
}

impl App {
    pub async fn build(config: &Settings) -> Self {
        let app_state = AppState {
            db_pool: get_connection_pool(&config.database),
            email_client: EmailClient::new(
                config.email_client.base_url.clone(),
                config
                    .email_client
                    .sender()
                    .expect("Invalid sender email address."),
                config.email_client.auth_token.clone(),
                config.email_client.timeout,
            ),
            base_url: config.application.base_url.clone(),
        };
        let app = Router::with_state(app_state)
            .route("/health_check", get(health_check))
            .route("/subscriptions", post(subscribe))
            .route("/subscriptions/confirm", get(confirm));

        let address = format!("{}:{}", config.application.host, config.application.port);
        let server = Server::bind(&address.parse().unwrap()).serve(app.into_make_service());

        Self { server }
    }

    pub async fn run(self) -> Result<(), Error> {
        self.server.await
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.server.local_addr()
    }
}

pub fn get_connection_pool(config: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy_with(config.with_db())
}
