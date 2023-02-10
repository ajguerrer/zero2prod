use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

use async_session::{Session, SessionStore};
use async_trait::async_trait;
use axum::http::{Request, StatusCode};
use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    middleware::Next,
    response::{IntoResponse, IntoResponseParts, Redirect, Response, ResponseParts},
    Extension,
};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    SignedCookieJar,
};
use uuid::Uuid;

use crate::{
    log::{OkOrWrapAndLog, WrapAndLogErr},
    startup::SessionState,
};

const USER_ID_KEY: &str = "user_id";
const SESSION_COOKIE_KEY: &str = "session_id";

pub struct UserSession(Session);

impl UserSession {
    pub fn renew(&mut self) {
        self.0.regenerate()
    }

    pub fn login(&mut self, user_id: Uuid) {
        self.0.insert(USER_ID_KEY, user_id).unwrap()
    }

    pub fn logout(&mut self) {
        self.0.destroy()
    }

    pub fn user_id(&self) -> Option<Uuid> {
        self.0.get(USER_ID_KEY)
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for UserSession
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            Extension::from_request_parts(parts, state)
                .await
                .expect("redis_session layer is not installed")
                .0,
        ))
    }
}

impl IntoResponseParts for UserSession {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.extensions_mut().insert(self.0);
        Ok(res)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UserId(pub Uuid);

impl Deref for UserId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UserId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for UserId
where
    S: Send + Sync,
{
    type Rejection = Redirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match UserSession::from_request_parts(parts, state)
            .await
            .map(|session| session.user_id())
        {
            Ok(Some(user_id)) => Ok(UserId(user_id)),
            _ => Err(Redirect::to("/login")),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct SessionError(#[from] anyhow::Error);

impl IntoResponse for SessionError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

pub async fn redis_session<B>(
    state: State<SessionState>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, SessionError> {
    let cookies = SignedCookieJar::from_headers(request.headers(), state.key.clone());
    let session = match cookies.get(SESSION_COOKIE_KEY) {
        Some(session_cookie) => state
            .redis_store
            .load_session(session_cookie.value().to_string())
            .await
            .wrap_and_log_err("Failed to load user session")?
            .ok_or_wrap_and_log("Failed to find user session")?,
        None => Session::new(),
    };

    request.extensions_mut().insert(session);

    let mut response = next.run(request).await;

    let cookies = match response.extensions_mut().remove::<Session>() {
        Some(session) if session.is_destroyed() => {
            state
                .redis_store
                .destroy_session(session)
                .await
                .wrap_and_log_err("Failed to cleanup user session")?;
            let mut cookie = Cookie::build(SESSION_COOKIE_KEY, "")
                .http_only(true)
                .path("/")
                .finish();
            cookie.make_removal();
            cookies.add(cookie)
        }
        Some(session) if session.data_changed() => {
            let cookie_value = state
                .redis_store
                .store_session(session)
                .await
                .wrap_and_log_err("Failed to store user session")?
                .ok_or_wrap_and_log("Failed to get session cookie")?;
            let cookie = Cookie::build(SESSION_COOKIE_KEY, cookie_value)
                .http_only(true)
                .same_site(SameSite::Strict)
                .path("/")
                .finish();
            cookies.add(cookie)
        }
        _ => cookies,
    };

    Ok((cookies, response).into_response())
}
