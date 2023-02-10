use axum::response::Html;
use axum_flash::IncomingFlashes;
use std::fmt::Write;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn login_form(flashes: IncomingFlashes) -> (IncomingFlashes, Html<String>) {
    let mut error_html = String::new();
    for (_, msg) in flashes.iter() {
        writeln!(error_html, "<p><i>{msg}</i></p>").unwrap();
    }
    let html = Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">

<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
</head>

<body>
    {error_html}
    <form action="/login" method="post">
        <label>Username
            <input type="text" placeholder="Enter Username" name="username">
        </label>

        <label>Password
            <input type="password" placeholder="Enter Password" name="password">
        </label>

        <button type="submit">Login</button>
    </form>
</body>

</html>
"#,
    ));

    (flashes, html)
}
