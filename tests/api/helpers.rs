use std::{env, net::SocketAddr};

use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use reqwest::{redirect::Policy, Client, Response, Url};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::{MockServer, Request};
use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
    email_client::EmailClient,
    issue_delivery_worker::{prune_idempotency_table, try_execute_task, ExecutionOutcome},
    startup::{get_connection_pool, App},
    telemetry::init_telemetry,
};

static TRACING: Lazy<()> = Lazy::new(|| {
    if env::var("TEST_LOG").is_ok() {
        init_telemetry("sqlx=error,info".into());
    }
});
pub struct TestApp {
    pub address: SocketAddr,
    pub db_pool: PgPool,
    pub email_server: MockServer,
    pub test_user: TestUser,
    pub api_client: Client,
    pub email_client: EmailClient,
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

pub struct ConfirmationLinks {
    pub html: Url,
    pub plain_text: Url,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> Response {
        self.api_client
            .post(format!("http://{}/subscriptions", self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub fn get_confirmation_links(&self, email_request: &Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            // Let's make sure we don't call random APIs on the web
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link
                .set_port(Some(self.address.port()))
                .unwrap();
            confirmation_link
        };

        let html = get_link(body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(body["TextBody"].as_str().unwrap());
        ConfirmationLinks { html, plain_text }
    }

    pub async fn get_publish_newsletter(&self) -> Response {
        self.api_client
            .get(&format!("http://{}/admin/newsletters", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_publish_newsletter_html(&self) -> String {
        self.get_publish_newsletter().await.text().await.unwrap()
    }

    pub async fn post_publish_newsletter<Body>(&self, body: &Body) -> Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("http://{}/admin/newsletters", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn login(&self) -> Response {
        self.post_login(&serde_json::json!({
            "username": &self.test_user.username,
            "password": &self.test_user.password,
        }))
        .await
    }

    pub async fn post_login<Body>(&self, body: &Body) -> Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("http://{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(&format!("http://{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
            .text()
            .await
            .unwrap()
    }

    pub async fn get_admin_dashboard(&self) -> Response {
        self.api_client
            .get(&format!("http://{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn get_change_password(&self) -> Response {
        self.api_client
            .get(&format!("http://{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_change_password_html(&self) -> String {
        self.get_change_password().await.text().await.unwrap()
    }

    pub async fn post_change_password<Body>(&self, body: &Body) -> Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("http://{}/admin/password", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_logout(&self) -> Response {
        self.api_client
            .post(&format!("http://{}/admin/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn dispatch_all_pending_emails(&self) {
        loop {
            if let ExecutionOutcome::EmptyQueue =
                try_execute_task(&self.db_pool, &self.email_client)
                    .await
                    .unwrap()
            {
                break;
            }
        }
    }

    pub async fn insert_idempotency_key(&self, created_on: DateTime<Utc>) {
        dbg!(&created_on);
        sqlx::query!(
            r#"
        INSERT INTO idempotency (
            user_id,
            idempotency_key,
            created_at
        )
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
            self.test_user.user_id,
            "1234",
            created_on
        )
        .execute(&self.db_pool)
        .await
        .unwrap();
    }

    pub async fn prune_idempotency_table(&self) -> u64 {
        prune_idempotency_table(&self.db_pool).await.unwrap()
    }
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let email_server = MockServer::start().await;

    let config = {
        let mut c = get_configuration().expect("Failed to read configuration");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.email_client.base_url = Url::parse(&email_server.uri()).unwrap();
        c
    };

    configure_database(&config.database).await;

    let app = App::build(&config)
        .await
        .expect("Failed to build application");
    let address = app.local_addr();
    tokio::spawn(app.run());

    let api_client = Client::builder()
        .redirect(Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let test_app = TestApp {
        address,
        db_pool: get_connection_pool(&config.database),
        email_server,
        test_user: TestUser::generate(),
        api_client,
        email_client: config.email_client.client(),
    };
    test_app.test_user.store(&test_app.db_pool).await;
    test_app
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");
    let db_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to migrate database");
    db_pool
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: "everything_has_to_start_somewhere".into(),
        }
    }

    async fn store(&self, db_pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::default()
            .hash_password(self.password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        sqlx::query!(
            r#"INSERT INTO users (user_id, username, password_hash)
            VALUES ($1, $2, $3)"#,
            self.user_id,
            self.username,
            password_hash,
        )
        .execute(db_pool)
        .await
        .expect("Failed to create test users");
    }
}

pub fn assert_is_redirect_to(response: &Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
