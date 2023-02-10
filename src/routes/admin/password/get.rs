use axum::response::{Html, Redirect};
use axum_flash::IncomingFlashes;
use std::fmt::Write;
use tracing::instrument;

use crate::user_session::UserId;

#[instrument(skip_all, fields(uuid=?*user_id))]
pub async fn change_password_form(
    flashes: IncomingFlashes,
    user_id: UserId,
) -> Result<(IncomingFlashes, Html<String>), Redirect> {
    let mut msg_html = String::new();
    for (_, msg) in flashes.iter() {
        writeln!(msg_html, "<p><i>{msg}</i></p>").unwrap();
    }

    Ok((
        flashes,
        Html(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Change Password</title>
</head>
<body>
    {msg_html}
    <form action="/admin/password" method="post">
        <label>Current password
            <input
                type="password"
                placeholder="Enter current password"
                name="current_password"
            >
            </label>
        <br>
        <label>New password
            <input
                type="password"
                placeholder="Enter new password"
                name="new_password"
            >
        </label>
        <br>
        <label>Confirm new password
            <input
                type="password"
                placeholder="Type the new password again"
                name="new_password_check"
            >
        </label>
        <br>
        <button type="submit">Change password</button>
    </form>
    <p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>"#,
        )),
    ))
}
