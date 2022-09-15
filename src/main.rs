use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use zero2prod::{configuration::get_configuration, startup::run, telemetry::init_telemetry};

#[tokio::main]
async fn main() {
    init_telemetry("info".into());

    let config = get_configuration().expect("Failed to read configuration.");
    let db_pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy_with(config.database.with_db());

    let address = format!("{}:{}", config.application.host, config.application.port);
    run(&address, db_pool).await.expect("Failed to run server.");
}
