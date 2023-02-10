use std::{net::SocketAddr, time::Duration};

use anyhow::Result;
use async_redis_session::RedisSessionStore;
use axum::{
    extract::FromRef,
    middleware::from_fn_with_state,
    routing::{get, post, IntoMakeService},
    Router, Server,
};
use axum_extra::extract::cookie::Key;
use axum_flash::Config;
use hyper::server::conn::AddrIncoming;
use secrecy::ExposeSecret;
use sqlx::{postgres::PgPoolOptions, PgPool};

use url::Url;

use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{
        admin_dashboard, change_password, change_password_form, confirm, health_check, home, login,
        login_form, logout, publish_newsletter, publish_newsletter_form, subscribe,
    },
    user_session::redis_session,
};

pub struct App {
    server: Server<AddrIncoming, IntoMakeService<Router>>,
}

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub email_client: EmailClient,
    pub base_url: Url,
    flash_config: Config,
}

impl FromRef<AppState> for Config {
    fn from_ref(state: &AppState) -> Self {
        state.flash_config.clone()
    }
}

#[derive(Clone)]
pub struct SessionState {
    pub redis_store: RedisSessionStore,
    pub key: Key,
}

impl App {
    pub async fn build(config: &Settings) -> Result<Self> {
        let key = Key::from(config.application.hmac_secret.expose_secret().as_bytes());
        let app_state = AppState {
            db_pool: get_connection_pool(&config.database),
            email_client: config.email_client.clone().client(),
            base_url: config.application.base_url.clone(),
            flash_config: Config::new(key.clone()),
        };
        let session_state = SessionState {
            redis_store: RedisSessionStore::new(config.redis_uri.expose_secret().as_ref())?,
            key,
        };
        let app = Router::new()
            .route("/", get(home))
            .route("/admin/dashboard", get(admin_dashboard))
            .route("/admin/newsletters", get(publish_newsletter_form))
            .route("/admin/newsletters", post(publish_newsletter))
            .route("/admin/password", get(change_password_form))
            .route("/admin/password", post(change_password))
            .route("/admin/logout", post(logout))
            .route("/health_check", get(health_check))
            .route("/login", get(login_form))
            .route("/login", post(login))
            .route("/subscriptions", post(subscribe))
            .route("/subscriptions/confirm", get(confirm))
            .route_layer(from_fn_with_state(session_state, redis_session))
            .with_state(app_state);

        let address = format!("{}:{}", config.application.host, config.application.port);
        let server = Server::bind(&address.parse().unwrap()).serve(app.into_make_service());

        Ok(Self { server })
    }

    pub async fn run(self) -> Result<()> {
        self.server.await?;

        Ok(())
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.server.local_addr()
    }
}

pub fn get_connection_pool(config: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(10))
        .connect_lazy_with(config.with_db())
}
