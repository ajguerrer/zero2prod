use tracing::{info, instrument};

#[instrument]
pub async fn health_check() {
    info!("healthy");
}
