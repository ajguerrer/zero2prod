use axum::response::Redirect;
use axum_flash::Flash;
use tracing::instrument;

use crate::user_session::{UserId, UserSession};

#[instrument(skip_all, fields(uuid=?*user_id))]
pub async fn logout(
    user_id: UserId,
    mut session: UserSession,
    mut flash: Flash,
) -> (UserSession, Flash, Redirect) {
    flash = flash.success("You have successfully logged out.");
    session.logout();

    (session, flash, Redirect::to("/login"))
}
