use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_telemetry(level: String) {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(level))
                .unwrap(),
        )
        .with(fmt::layer().with_target(false))
        .with(ErrorLayer::default())
        .init();
}
