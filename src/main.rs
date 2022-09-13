use secrecy::ExposeSecret;
use sqlx::PgPool;
use zero2prod::{configuration::get_configuration, startup::run, telemetry::init_telemetry};

#[tokio::main]
async fn main() {
    init_telemetry("info,sqlx=warn".into());

    let config = get_configuration().expect("Failed to read configuration.");
    let db_pool = PgPool::connect(config.database.connection_string().expose_secret())
        .await
        .expect("Failed to connect to Postgres.");

    let port = config.application_port;
    run(&format!("127.0.0.1:{port}"), db_pool)
        .await
        .expect("Failed to run server.");
}
