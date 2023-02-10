use uuid::Uuid;

use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn you_must_be_logged_in_to_see_the_change_password_form() {
    let app = spawn_app().await;

    let response = app.get_change_password().await;

    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn you_must_be_logged_in_to_change_your_password() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();

    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": Uuid::new_v4().to_string(),
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn new_password_fields_must_match() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let another_new_password = Uuid::new_v4().to_string();

    app.login().await;

    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &another_new_password,
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains(
        "<p><i>You entered two different new passwords - \
         the field values must match.</i></p>"
    ));
}

#[tokio::test]
async fn new_password_must_not_be_too_short() {
    let app = spawn_app().await;
    let short_password = "twelve_chars";

    app.login().await;

    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &short_password,
            "new_password_check": &short_password,
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>The new password is too short.</i></p>"));
}

#[tokio::test]
async fn new_password_must_not_be_too_long() {
    let app = spawn_app().await;
    let long_password = "this_password_is_way_too_long_clocking_in_at_a_lengthy_one_hundred_and_twenty_eight_chars_how_could_anyone_remember_such_a_thing";

    app.login().await;

    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &long_password,
            "new_password_check": &long_password,
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>The new password is too long.</i></p>"));
}

#[tokio::test]
async fn current_password_must_be_valid() {
    let app = spawn_app().await;

    app.login().await;

    let new_password = Uuid::new_v4().to_string();
    let wrong_password = Uuid::new_v4().to_string();
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &wrong_password,
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>The current password is incorrect.</i></p>"));
}

#[tokio::test]
async fn changing_password_works() {
    let app = spawn_app().await;

    let response = app.login().await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let new_password = Uuid::new_v4().to_string();
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>Your password has been changed.</i></p>"));

    let response = app.post_logout().await;
    assert_is_redirect_to(&response, "/login");

    let html_page = dbg!(app.get_login_html().await);
    assert!(html_page.contains("<p><i>You have successfully logged out.</i></p>"));

    let response = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &new_password
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/dashboard")
}
