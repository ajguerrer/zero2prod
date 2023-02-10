use axum::response::Html;
use axum_flash::IncomingFlashes;
use std::fmt::Write;

use crate::user_session::UserId;

#[tracing::instrument(skip_all, fields(uuid=?*user_id))]
pub async fn publish_newsletter_form(
    user_id: UserId,
    flashes: IncomingFlashes,
) -> (IncomingFlashes, Html<String>) {
    let mut msg_html = String::new();
    for (_, msg) in flashes.iter() {
        writeln!(msg_html, "<p><i>{msg}</i></p>").unwrap();
    }
    let idempotency_key = uuid::Uuid::new_v4();
    let html = Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Publish Newsletter Issue</title>
</head>
<body>
    {msg_html}
    <form action="/admin/newsletters" method="post">
        <label>Title:<br>
            <input
                type="text"
                placeholder="Enter the issue title"
                name="title"
            >
        </label>
        <br>
        <label>Plain text content:<br>
            <textarea
                placeholder="Enter the content in plain text"
                name="text_content"
                rows="20"
                cols="50"
            ></textarea>
        </label>
        <br>
        <label>HTML content:<br>
            <textarea
                placeholder="Enter the content in HTML format"
                name="html_content"
                rows="20"
                cols="50"
            ></textarea>
        </label>
        <br>
        <input hidden type="text" name="idempotency_key" value="{idempotency_key}">
        <button type="submit">Publish</button>
    </form>
    <p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>"#,
    ));

    (flashes, html)
}
