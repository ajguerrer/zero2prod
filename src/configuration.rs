use std::time::Duration;

use config::{Config, ConfigError, Environment, File};
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use serde_aux::prelude::deserialize_number_from_string;
use serde_with::{serde_as, DurationMilliSeconds};
use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    ConnectOptions,
};
use url::Url;

use crate::domain::SubscriberEmail;

#[derive(Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,
    pub database: DatabaseSettings,
    pub email_client: EmailClientSettings,
}

#[derive(Deserialize)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub base_url: Url,
}

#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
}

#[serde_as]
#[derive(Deserialize)]
pub struct EmailClientSettings {
    pub base_url: Url,
    pub sender_email: String,
    pub auth_token: Secret<String>,
    #[serde(rename = "timeout_millis")]
    #[serde_as(as = "DurationMilliSeconds")]
    pub timeout: Duration,
}

pub fn get_configuration() -> Result<Settings, ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine current directory.");
    let config_dir = base_path.join("config");
    let env = std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "local".into());
    let settings = Config::builder()
        .add_source(File::from(config_dir.join("base.yaml")))
        .add_source(File::from(config_dir.join(format!("{}.yaml", env))))
        .add_source(
            Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;
    settings.try_deserialize()
}

impl DatabaseSettings {
    pub fn with_db(&self) -> PgConnectOptions {
        let mut options = self.without_db().database(&self.database_name);
        options.log_statements(tracing::log::LevelFilter::Trace);
        options
    }

    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode)
    }
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }
}
