use reqwest::Client;

use crate::helpers::spawn_app;

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;
    let address = app.address;
    let client = Client::new();

    let response = client
        .get(format!("http://{address}/health_check"))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success());
}
