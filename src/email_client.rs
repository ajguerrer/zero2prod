use std::time::Duration;

use reqwest::{Client, Error, Url};
use secrecy::{ExposeSecret, Secret};
use serde::Serialize;

use crate::domain::SubscriberEmail;

#[derive(Debug, Clone)]
pub struct EmailClient {
    http_client: Client,
    base_url: Url,
    sender: SubscriberEmail,
    auth_token: Secret<String>,
}

impl EmailClient {
    pub fn new(
        base_url: Url,
        sender: SubscriberEmail,
        auth_token: Secret<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            http_client: Client::builder().timeout(timeout).build().unwrap(),
            base_url,
            sender,
            auth_token,
        }
    }

    pub async fn send(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_body: &str,
        text_body: &str,
    ) -> Result<(), Error> {
        let request_body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject,
            html_body,
            text_body,
        };
        self.http_client
            .post(self.base_url.join("email").unwrap())
            .bearer_auth(self.auth_token.expose_secret())
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use fake::{
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
        Fake, Faker,
    };
    use reqwest::Url;
    use secrecy::Secret;
    use tokio_test::{assert_err, assert_ok};
    use wiremock::{
        matchers::{any, header, header_exists, method, path},
        Match, Mock, MockServer, Request, ResponseTemplate,
    };

    use crate::{domain::SubscriberEmail, email_client::EmailClient};

    #[tokio::test]
    async fn send_email_fires_a_request_to_url() {
        let (email_client, mock_server) = client_and_mock_server().await;

        Mock::given(header_exists("Authorization"))
            .and(header("Content-Type", "application/json"))
            .and(path("/email"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _ = email_client
            .send(&email(), &subject(), &content(), &content())
            .await;
    }

    #[tokio::test]
    async fn send_email_succeed_if_the_server_returns_200() {
        let (email_client, mock_server) = client_and_mock_server().await;

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        assert_ok!(
            email_client
                .send(&email(), &subject(), &content(), &content())
                .await
        );
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        let (email_client, mock_server) = client_and_mock_server().await;

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        assert_err!(
            email_client
                .send(&email(), &subject(), &content(), &content())
                .await
        );
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        let (email_client, mock_server) = client_and_mock_server().await;

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(180)))
            .expect(1)
            .mount(&mock_server)
            .await;

        assert_err!(
            email_client
                .send(&email(), &subject(), &content(), &content())
                .await
        );
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn content() -> String {
        Paragraph(1..10).fake()
    }

    async fn client_and_mock_server() -> (EmailClient, MockServer) {
        let mock_server = MockServer::start().await;
        let email_client = EmailClient::new(
            Url::parse(&mock_server.uri()).unwrap(),
            email(),
            Secret::new(Faker.fake()),
            Duration::from_millis(100),
        );

        (email_client, mock_server)
    }

    struct SendEmailBodyMatcher;
    impl Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            // Don't rely on deserialize, check the raw value
            if let Ok(serde_json::Value::Object(body)) = serde_json::from_slice(&request.body) {
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            } else {
                false
            }
        }
    }
}
