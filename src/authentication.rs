use anyhow::Context;
use argon2::{
    password_hash::SaltString, Algorithm, Argon2, Params, PasswordHash, PasswordHasher,
    PasswordVerifier, Version,
};
use axum::headers::{authorization::Basic, Authorization};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use tokio::task::spawn_blocking;
use tracing::{warn, Span};
use uuid::Uuid;

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
    #[error("Invalid credentials")]
    AuthError(argon2::password_hash::Error),
}

impl From<Authorization<Basic>> for Credentials {
    fn from(header: Authorization<Basic>) -> Self {
        Credentials {
            username: header.username().to_string(),
            password: Secret::new(header.password().to_string()),
        }
    }
}

#[tracing::instrument(skip_all)]
pub async fn validate_credentials(
    credentials: Credentials,
    db_pool: &PgPool,
) -> Result<Uuid, AuthError> {
    let (user_id, password_hash) = get_user_credentials(credentials.username, db_pool).await?;
    spawn_blocking(move || {
        Span::current().in_scope(|| verify_password_hash(credentials.password, password_hash))
    })
    .await
    .context("Failed to spawn blocking password verification task")??;
    Ok(user_id)
}

const FALLBACK_PASSWORD_HASH: &str = "$argon2id$v=19$m=15000,t=2,p=1$\
gZiV/M1gPc22ElAH/Jh1Hw$\
CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno";

#[tracing::instrument(skip_all)]
async fn get_user_credentials(
    username: String,
    db_pool: &PgPool,
) -> Result<(Uuid, Secret<String>), AuthError> {
    let user_id = sqlx::query!(
        r#"SELECT user_id, password_hash FROM users WHERE username = $1"#,
        username,
    )
    .fetch_optional(db_pool)
    .await
    .context("Failed to query auth credentials")?;

    let (user_id, password_hash) = user_id
        .map(|r| (r.user_id, Secret::new(r.password_hash)))
        .unwrap_or_else(|| {
            warn!("invalid username, returning fallback password hash to prevent timing attacks");
            (
                Uuid::new_v4(),
                Secret::new(FALLBACK_PASSWORD_HASH.to_string()),
            )
        });

    Ok((user_id, password_hash))
}

#[tracing::instrument(skip_all)]
pub fn verify_password_hash(
    password: Secret<String>,
    password_hash: Secret<String>,
) -> Result<(), AuthError> {
    let password_hash =
        PasswordHash::new(password_hash.expose_secret()).context("Failed to parse PHC hash")?;
    Argon2::default()
        .verify_password(password.expose_secret().as_bytes(), &password_hash)
        .map_err(AuthError::AuthError)?;

    Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn change_password(
    user_id: Uuid,
    password: Secret<String>,
    db_pool: &PgPool,
) -> Result<(), AuthError> {
    let password_hash =
        spawn_blocking(move || Span::current().in_scope(|| compute_password_hash(password)))
            .await
            .context("Failed to spawn blocking password hashing task")??;

    sqlx::query!(
        r#"
            UPDATE users 
            SET password_hash = $1 
            WHERE user_id = $2
            "#,
        password_hash.expose_secret(),
        user_id
    )
    .execute(db_pool)
    .await
    .context("Failed to execute password change query.")?;

    Ok(())
}

fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, AuthError> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)
    .map_err(AuthError::AuthError)?;

    Ok(Secret::new(password_hash.to_string()))
}
