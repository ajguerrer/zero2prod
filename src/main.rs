use hyper::Error;
use zero2prod::{configuration::get_configuration, startup::App, telemetry::init_telemetry};

#[tokio::main]
async fn main() -> Result<(), Error> {
    init_telemetry("sqlx=error,info".into());

    let config = get_configuration().expect("Failed to read configuration.");
    let app = App::build(&config).await;
    app.run().await
}
