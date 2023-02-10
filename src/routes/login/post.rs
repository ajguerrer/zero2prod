use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_flash::Flash;
use hyper::StatusCode;
use secrecy::Secret;
use serde::Deserialize;
use tracing::{field::debug, instrument, Span};

use crate::{
    authentication::{validate_credentials, AuthError, Credentials},
    log::LogErr,
    startup::AppState,
    user_session::UserSession,
};

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: Secret<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError,
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl From<AuthError> for LoginError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::UnexpectedError(err) => LoginError::UnexpectedError(err),
            AuthError::AuthError(_) => LoginError::AuthError,
        }
    }
}

impl IntoResponse for LoginError {
    fn into_response(self) -> Response {
        match self {
            LoginError::AuthError => Redirect::to("/login").into_response(),
            LoginError::UnexpectedError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
        }
    }
}

#[instrument(skip_all, fields(user=form.username, uuid))]
pub async fn login(
    state: State<AppState>,
    mut session: UserSession,
    flash: Flash,
    form: Form<LoginForm>,
) -> Result<(UserSession, Redirect), (Flash, LoginError)> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };

    match validate_credentials(credentials, &state.db_pool)
        .await
        .log_err()
    {
        Ok(user_id) => {
            Span::current().record("uuid", debug(&user_id));

            session.renew();
            session.login(user_id);

            Ok((session, Redirect::to("/admin/dashboard")))
        }
        Err(err) => {
            let err = LoginError::from(err);
            Err((flash.error(err.to_string()), err))
        }
    }
}
