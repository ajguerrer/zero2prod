use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_flash::Flash;
use hyper::StatusCode;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    authentication::{validate_credentials, AuthError, Credentials},
    log::{LogErr, WrapAndLogErr},
    routes::admin::dashboard::get_username,
    startup::AppState,
    user_session::UserId,
};

#[derive(Deserialize)]
pub struct ChangePassword {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum ChangePasswordError {
    #[error("Password invalid")]
    Invalid(Flash),
    #[error("Authentication failed")]
    AuthError,
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl From<AuthError> for ChangePasswordError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::UnexpectedError(err) => ChangePasswordError::UnexpectedError(err),
            AuthError::AuthError(_) => ChangePasswordError::AuthError,
        }
    }
}

impl IntoResponse for ChangePasswordError {
    fn into_response(self) -> Response {
        match self {
            ChangePasswordError::Invalid(flash) => {
                (flash, Redirect::to("/admin/password")).into_response()
            }
            ChangePasswordError::AuthError => Redirect::to("/admin/password").into_response(),
            ChangePasswordError::UnexpectedError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
        }
    }
}

#[instrument(skip_all, fields(uuid=?*user_id))]
pub async fn change_password(
    state: State<AppState>,
    user_id: UserId,
    flash: Flash,
    form: Form<ChangePassword>,
) -> Result<(Flash, Redirect), ChangePasswordError> {
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        return Err(ChangePasswordError::Invalid(flash.error(
            "You entered two different new passwords - the field values must match.",
        )));
    }

    const PASSWORD_LOWER_BOUND: usize = 13;
    if form.new_password.expose_secret().len() <= PASSWORD_LOWER_BOUND {
        return Err(ChangePasswordError::Invalid(
            flash.error("The new password is too short."),
        ));
    }

    const PASSWORD_UPPER_BOUND: usize = 128;
    if form.new_password.expose_secret().len() >= PASSWORD_UPPER_BOUND {
        return Err(ChangePasswordError::Invalid(
            flash.error("The new password is too long."),
        ));
    }

    let username = get_username(*user_id, &state.db_pool)
        .await
        .wrap_and_log_err("Failed to query username")?;

    let credentials = Credentials {
        username,
        password: form.0.current_password,
    };

    if validate_credentials(credentials, &state.db_pool)
        .await
        .log_err()
        .is_err()
    {
        return Err(ChangePasswordError::Invalid(
            flash.error("The current password is incorrect."),
        ));
    };

    crate::authentication::change_password(*user_id, form.0.new_password, &state.db_pool)
        .await
        .wrap_and_log_err("Failed to change password.")?;

    Ok((
        flash.success("Your password has been changed."),
        Redirect::to("/admin/password"),
    ))
}
