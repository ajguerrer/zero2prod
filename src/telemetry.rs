use tracing_error::ErrorLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_telemetry(level: String) {
    color_eyre::install().expect("Failed to init telemetry.");
    tracing_subscriber::registry()
        .with(EnvFilter::new(std::env::var("RUST_LOG").unwrap_or(level)))
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer())
        .init();
}
