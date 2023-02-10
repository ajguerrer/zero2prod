use tracing_error::ErrorLayer;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn init_telemetry(level: String) {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(level))
                .unwrap(),
        )
        .with(ErrorLayer::default())
        .with(
            fmt::layer()
                .with_span_events(FmtSpan::CLOSE)
                .with_target(false),
        )
        .init();
}
