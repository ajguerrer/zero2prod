use anyhow::Result;
use zero2prod::{
    configuration::get_configuration, issue_delivery_worker::run_worker_until_stopped,
    startup::App, telemetry::init_telemetry,
};

#[tokio::main]
async fn main() -> Result<()> {
    init_telemetry("sqlx=error,info".into());

    let config = get_configuration().expect("Failed to read configuration");
    let app = tokio::spawn(App::build(&config).await?.run());
    let worker = tokio::spawn(run_worker_until_stopped(config));
    tokio::select! {
        _ = app => {},
        _ = worker => {},
    };

    Ok(())
}
