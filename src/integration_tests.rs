use actix_web::cookie::Cookie;
use actix_web::http::StatusCode;
use actix_web::{test, web};
use chrono::{Duration, Local};
use diesel::SelectableHelper;
use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::AppState;
use crate::auth;
use crate::configure_app;
use crate::db;
use crate::models::post::{NewPost, POST_APPROVED, POST_PENDING_APPROVAL, Post};
use crate::models::survey_response::{NewSurveyResponse, SurveyResponse};
use crate::models::user::{
    ACCOUNT_ACTIVE, ACCOUNT_LOCKED, ACCOUNT_PENDING, ACCOUNT_SUSPENDED, NewUser, User,
};
use crate::schema::{posts, survey_responses, users};

const TEST_PASSWORD: &str = "Correct Horse Battery Staple";
const CSRF: &str = "test-csrf-token";

#[derive(Clone)]
struct SeededUser {
    id: String,
    token_version: i32,
    account_status: String,
}

fn test_state() -> web::Data<AppState> {
    let db_path = std::env::temp_dir().join(format!("eq-site-test-{}.db", Uuid::now_v7()));
    let db_url = db_path.to_string_lossy().to_string();
    let db_pool = db::create_db_pool(&db_url, 8);
    let mut conn = db_pool.get().expect("db conn");
    db::run_migrations(&mut conn).expect("migrations");
    drop(conn);

    web::Data::new(AppState { db_pool })
}

fn seed_user(
    state: &web::Data<AppState>,
    email: &str,
    is_admin: bool,
    account_status: &str,
    must_change_password: bool,
) -> SeededUser {
    let now = db::now_ts();
    let id = Uuid::now_v7().to_string();
    let password_hash = auth::hash_password(TEST_PASSWORD).expect("hash password");
    let mut conn = state.db_pool.get().expect("db conn");
    diesel::insert_into(users::table)
        .values(NewUser {
            id: &id,
            email,
            first_name: "Test",
            last_name: "User",
            password_hash: &password_hash,
            token_version: 0,
            is_admin,
            account_status,
            must_change_password,
            created_at: now,
            updated_at: now,
        })
        .execute(&mut conn)
        .expect("insert user");
    SeededUser {
        id,
        token_version: 0,
        account_status: account_status.to_string(),
    }
}

fn csrf_cookie() -> Cookie<'static> {
    Cookie::build("xsrf-token", CSRF).path("/").finish()
}

fn access_cookie(user: &SeededUser) -> Cookie<'static> {
    Cookie::build(
        "access_token",
        auth::create_access_token(&user.id, user.token_version, &user.account_status)
            .expect("access token"),
    )
    .path("/")
    .finish()
}

fn refresh_cookie(user: &SeededUser) -> Cookie<'static> {
    Cookie::build(
        "refresh_token",
        auth::create_refresh_token(&user.id, user.token_version, &user.account_status)
            .expect("refresh token"),
    )
    .path("/")
    .finish()
}

fn relative_date(days: i64) -> String {
    (Local::now().date_naive() + Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

fn user_row(state: &web::Data<AppState>, user_id: &str) -> User {
    let mut conn = state.db_pool.get().expect("db conn");
    users::table
        .find(user_id)
        .select(User::as_select())
        .first::<User>(&mut conn)
        .expect("load user")
}

fn post_row(state: &web::Data<AppState>, post_id: &str) -> Post {
    let mut conn = state.db_pool.get().expect("db conn");
    posts::table
        .find(post_id)
        .select(Post::as_select())
        .first::<Post>(&mut conn)
        .expect("load post")
}

fn seed_post(
    state: &web::Data<AppState>,
    user: &SeededUser,
    body: &str,
    created_at: i64,
) -> String {
    let id = Uuid::now_v7().to_string();
    let mut conn = state.db_pool.get().expect("db conn");
    diesel::insert_into(posts::table)
        .values(NewPost {
            id: &id,
            author_user_id: Some(&user.id),
            is_anonymous: false,
            approval_status: POST_APPROVED,
            body,
            created_at,
            updated_at: created_at,
        })
        .execute(&mut conn)
        .expect("insert post");
    id
}

fn survey_response_row(state: &web::Data<AppState>, response_id: &str) -> SurveyResponse {
    let mut conn = state.db_pool.get().expect("db conn");
    survey_responses::table
        .find(response_id)
        .select(SurveyResponse::as_select())
        .first::<SurveyResponse>(&mut conn)
        .expect("load survey response")
}

#[actix_web::test]
async fn sign_up_and_sign_in_use_server_side_password_hash() {
    let state = test_state();
    let app = test::init_service(configure_app(state.clone())).await;

    let sign_up = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/sign-up")
            .set_json(json!({
                "first_name": "Alice",
                "last_name": "Member",
                "email": "alice@example.com",
                "password": TEST_PASSWORD
            }))
            .to_request(),
    )
    .await;
    assert_eq!(sign_up.status(), StatusCode::OK);
    let body: Value = test::read_body_json(sign_up).await;

    let user = user_row(&state, body["user"]["id"].as_str().unwrap());
    assert!(auth::verify_password(TEST_PASSWORD, &user.password_hash).unwrap());
    assert_ne!(user.password_hash, TEST_PASSWORD);

    let sign_in = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/sign-in")
            .set_json(json!({
                "email": "alice@example.com",
                "password": TEST_PASSWORD
            }))
            .to_request(),
    )
    .await;
    assert_eq!(sign_in.status(), StatusCode::OK);
}

#[actix_web::test]
async fn sign_in_missing_email_uses_generic_unauthorized_response() {
    let state = test_state();
    let user = seed_user(&state, "present@example.com", false, ACCOUNT_ACTIVE, false);
    let app = test::init_service(configure_app(state.clone())).await;

    let wrong_password = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/sign-in")
            .set_json(json!({
                "email": "present@example.com",
                "password": "wrong password"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(wrong_password.status(), StatusCode::UNAUTHORIZED);
    let wrong_password_body: Value = test::read_body_json(wrong_password).await;

    let missing_email = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/sign-in")
            .set_json(json!({
                "email": "missing@example.com",
                "password": TEST_PASSWORD
            }))
            .to_request(),
    )
    .await;
    assert_eq!(missing_email.status(), StatusCode::UNAUTHORIZED);
    let missing_email_body: Value = test::read_body_json(missing_email).await;
    assert_eq!(missing_email_body, wrong_password_body);

    let still_can_sign_in = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/sign-in")
            .set_json(json!({
                "email": "present@example.com",
                "password": TEST_PASSWORD
            }))
            .to_request(),
    )
    .await;
    assert_eq!(still_can_sign_in.status(), StatusCode::OK);
    assert_eq!(user_row(&state, &user.id).email, "present@example.com");
}

#[actix_web::test]
async fn refresh_rotates_auth_cookies_and_revokes_used_refresh_token() {
    let state = test_state();
    let user = seed_user(&state, "refresh@example.com", false, ACCOUNT_ACTIVE, false);
    let app = test::init_service(configure_app(state.clone())).await;
    let old_refresh_cookie = refresh_cookie(&user);

    let refreshed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/refresh")
            .cookie(old_refresh_cookie.clone())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({}))
            .to_request(),
    )
    .await;
    assert_eq!(refreshed.status(), StatusCode::OK);

    let cookie_names = refreshed
        .response()
        .cookies()
        .map(|cookie| cookie.name().to_string())
        .collect::<Vec<_>>();
    assert!(cookie_names.contains(&"access_token".to_string()));
    assert!(cookie_names.contains(&"refresh_token".to_string()));
    assert!(cookie_names.contains(&"xsrf-token".to_string()));

    let reused = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/refresh")
            .cookie(old_refresh_cookie)
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({}))
            .to_request(),
    )
    .await;
    assert_eq!(reused.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn change_password_updates_password_hash_and_invalidates_old_password() {
    let state = test_state();
    let user = seed_user(&state, "change@example.com", false, ACCOUNT_ACTIVE, false);
    let app = test::init_service(configure_app(state.clone())).await;
    let new_password = "New Correct Horse Battery Staple";

    let changed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/change-password")
            .cookie(access_cookie(&user))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "current_password": TEST_PASSWORD,
                "new_password": new_password
            }))
            .to_request(),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::OK);

    let updated = user_row(&state, &user.id);
    assert!(auth::verify_password(new_password, &updated.password_hash).unwrap());
    assert!(!auth::verify_password(TEST_PASSWORD, &updated.password_hash).unwrap());
    assert_eq!(updated.token_version, 1);
}

#[actix_web::test]
async fn anonymous_post_endpoint_without_auth_creates_pending_unowned_post() {
    let state = test_state();
    let user = seed_user(&state, "anon@example.com", false, ACCOUNT_ACTIVE, false);
    let app = test::init_service(configure_app(state.clone())).await;

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=true")
            .peer_addr("127.0.0.10:12345".parse().unwrap())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Anonymous body" }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(created).await;
    assert_eq!(body["is_anonymous"], true);
    assert_eq!(body["approval_status"], POST_PENDING_APPROVAL);
    assert!(body["author_user_id"].is_null());
    assert!(body["author_name"].is_null());

    let post = post_row(&state, body["id"].as_str().unwrap());
    assert!(post.is_anonymous);
    assert!(post.author_user_id.is_none());
    assert_eq!(post.approval_status, POST_PENDING_APPROVAL);

    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/feed/posts")
            .cookie(access_cookie(&user))
            .to_request(),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body: Value = test::read_body_json(listed).await;
    assert_eq!(listed_body["posts"].as_array().unwrap().len(), 0);

    let thread = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/feed/posts/{}", body["id"].as_str().unwrap()))
            .cookie(access_cookie(&user))
            .to_request(),
    )
    .await;
    assert_eq!(thread.status(), StatusCode::NOT_FOUND);

    let reply = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "/api/feed/posts/{}/replies",
                body["id"].as_str().unwrap()
            ))
            .cookie(access_cookie(&user))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Reply" }))
            .to_request(),
    )
    .await;
    assert_eq!(reply.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn anonymous_post_endpoint_uses_optional_auth_status_and_rate_limits_by_ip() {
    let state = test_state();
    let active = seed_user(&state, "active@example.com", false, ACCOUNT_ACTIVE, false);
    let pending = seed_user(&state, "pending@example.com", false, ACCOUNT_PENDING, false);
    let suspended = seed_user(
        &state,
        "suspended@example.com",
        false,
        ACCOUNT_SUSPENDED,
        false,
    );
    let locked = seed_user(&state, "locked@example.com", false, ACCOUNT_LOCKED, false);
    let app = test::init_service(configure_app(state.clone())).await;

    for (index, user) in [suspended, locked].into_iter().enumerate() {
        let denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/feed/posts?anonymous=true")
                .peer_addr(format!("127.0.0.{}:12345", 20 + index).parse().unwrap())
                .cookie(access_cookie(&user))
                .cookie(csrf_cookie())
                .insert_header(("x-xsrf-token", CSRF))
                .set_json(json!({ "body": "Nope" }))
                .to_request(),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    }

    let active_created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=true")
            .peer_addr("127.0.0.30:12345".parse().unwrap())
            .cookie(access_cookie(&active))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "First" }))
            .to_request(),
    )
    .await;
    assert_eq!(active_created.status(), StatusCode::CREATED);
    let active_body: Value = test::read_body_json(active_created).await;
    assert_eq!(active_body["approval_status"], POST_APPROVED);
    assert_eq!(
        post_row(&state, active_body["id"].as_str().unwrap()).approval_status,
        POST_APPROVED
    );

    let pending_created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=true")
            .peer_addr("127.0.0.31:12345".parse().unwrap())
            .cookie(access_cookie(&pending))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Pending" }))
            .to_request(),
    )
    .await;
    assert_eq!(pending_created.status(), StatusCode::CREATED);
    let pending_body: Value = test::read_body_json(pending_created).await;
    assert_eq!(pending_body["approval_status"], POST_PENDING_APPROVAL);

    let second_same_ip = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=false")
            .peer_addr("127.0.0.30:23456".parse().unwrap())
            .cookie(access_cookie(&active))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Second" }))
            .to_request(),
    )
    .await;
    assert_eq!(second_same_ip.status(), StatusCode::CREATED);

    let third_same_ip = test::try_call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=true")
            .peer_addr("127.0.0.30:34567".parse().unwrap())
            .cookie(access_cookie(&active))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Third" }))
            .to_request(),
    )
    .await;
    let status = match third_same_ip {
        Ok(response) => response.status(),
        Err(error) => error.as_response_error().status_code(),
    };
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[actix_web::test]
async fn anonymous_post_endpoint_rejects_invalid_access_cookie() {
    let state = test_state();
    let app = test::init_service(configure_app(state)).await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=true")
            .peer_addr("127.0.0.40:12345".parse().unwrap())
            .cookie(Cookie::build("access_token", "not-a-valid-token").finish())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Anonymous body" }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn cannot_remove_admin_privileges_from_the_last_admin() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "last-admin-role@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state.clone())).await;

    let demoted = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/admin/users/{}/role", admin.id))
            .cookie(access_cookie(&admin))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "is_admin": false }))
            .to_request(),
    )
    .await;

    assert_eq!(demoted.status(), StatusCode::CONFLICT);
    assert!(user_row(&state, &admin.id).is_admin);
}

#[actix_web::test]
async fn cannot_delete_the_last_admin_user() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "last-admin-delete@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state.clone())).await;

    let deleted = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/api/admin/users/{}", admin.id))
            .cookie(access_cookie(&admin))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .to_request(),
    )
    .await;

    assert_eq!(deleted.status(), StatusCode::CONFLICT);
    assert!(user_row(&state, &admin.id).is_admin);
}

#[actix_web::test]
async fn admin_can_approve_pending_anonymous_post_for_feed_visibility() {
    let state = test_state();
    let admin = seed_user(&state, "admin@example.com", true, ACCOUNT_ACTIVE, false);
    let member = seed_user(&state, "member@example.com", false, ACCOUNT_ACTIVE, false);
    let app = test::init_service(configure_app(state.clone())).await;

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=true")
            .peer_addr("127.0.0.50:12345".parse().unwrap())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Needs approval" }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body: Value = test::read_body_json(created).await;
    let post_id = created_body["id"].as_str().unwrap();

    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/anonymous-posts/pending")
            .cookie(access_cookie(&admin))
            .to_request(),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body: Value = test::read_body_json(listed).await;
    assert_eq!(listed_body["posts"][0]["id"], post_id);

    let non_admin_list = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/anonymous-posts/pending")
            .cookie(access_cookie(&member))
            .to_request(),
    )
    .await;
    assert_eq!(non_admin_list.status(), StatusCode::FORBIDDEN);

    let approved = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/admin/anonymous-posts/{post_id}/approve"))
            .cookie(access_cookie(&admin))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .to_request(),
    )
    .await;
    assert_eq!(approved.status(), StatusCode::OK);
    assert_eq!(post_row(&state, post_id).approval_status, POST_APPROVED);

    let feed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/feed/posts")
            .cookie(access_cookie(&member))
            .to_request(),
    )
    .await;
    assert_eq!(feed.status(), StatusCode::OK);
    let feed_body: Value = test::read_body_json(feed).await;
    assert_eq!(feed_body["posts"][0]["id"], post_id);
}

#[actix_web::test]
async fn admin_event_endpoint_accepts_optional_end_fields() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "event-admin@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state.clone())).await;

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/admin/events")
            .cookie(access_cookie(&admin))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "name": "Date range activity",
                "event_date": relative_date(1),
                "event_time": null,
                "end_date": relative_date(3),
                "end_time": null,
                "location": "",
                "description": ""
            }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(created).await;
    assert_eq!(body["event_time"], Value::Null);
    assert_eq!(body["end_date"], relative_date(3));
    assert_eq!(body["end_time"], Value::Null);

    let event_id = body["id"].as_str().unwrap();
    let updated = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/api/admin/events/{event_id}"))
            .cookie(access_cookie(&admin))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "name": "Same-day activity",
                "event_date": relative_date(2),
                "event_time": "18:00",
                "end_date": null,
                "end_time": "20:00",
                "location": null,
                "description": null
            }))
            .to_request(),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let body: Value = test::read_body_json(updated).await;
    assert_eq!(body["event_time"], "18:00");
    assert_eq!(body["end_date"], Value::Null);
    assert_eq!(body["end_time"], "20:00");
}

#[actix_web::test]
async fn admin_event_endpoint_rejects_end_before_start() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "event-validation-admin@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state.clone())).await;

    let invalid_date = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/admin/events")
            .cookie(access_cookie(&admin))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "name": "Invalid date range",
                "event_date": relative_date(2),
                "event_time": null,
                "end_date": relative_date(1),
                "end_time": null
            }))
            .to_request(),
    )
    .await;
    assert_eq!(invalid_date.status(), StatusCode::BAD_REQUEST);

    let invalid_time = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/admin/events")
            .cookie(access_cookie(&admin))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "name": "Invalid time range",
                "event_date": relative_date(2),
                "event_time": "20:00",
                "end_date": null,
                "end_time": "18:00"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(invalid_time.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn landing_includes_ongoing_multiday_events() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "event-landing-admin@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state.clone())).await;

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/admin/events")
            .cookie(access_cookie(&admin))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "name": "Ongoing activity",
                "event_date": relative_date(-1),
                "event_time": null,
                "end_date": relative_date(1),
                "end_time": null
            }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let event: Value = test::read_body_json(created).await;

    let landing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/public/landing")
            .to_request(),
    )
    .await;
    assert_eq!(landing.status(), StatusCode::OK);
    let body: Value = test::read_body_json(landing).await;
    assert_eq!(body["upcoming_events"][0]["id"], event["id"]);
    assert_eq!(body["upcoming_events"][0]["end_date"], relative_date(1));
}

#[actix_web::test]
async fn regular_post_endpoint_rejects_anonymous_payload_shape() {
    let state = test_state();
    let user = seed_user(&state, "regular@example.com", false, ACCOUNT_ACTIVE, false);
    let app = test::init_service(configure_app(state)).await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts")
            .cookie(access_cookie(&user))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "body": "Regular body",
                "anonymous": true
            }))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn unified_post_endpoint_creates_named_posts_by_default_and_explicit_false() {
    let state = test_state();
    let user = seed_user(&state, "named@example.com", false, ACCOUNT_ACTIVE, false);
    let app = test::init_service(configure_app(state.clone())).await;

    let default_created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts")
            .cookie(access_cookie(&user))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Default named body" }))
            .to_request(),
    )
    .await;
    assert_eq!(default_created.status(), StatusCode::CREATED);
    let default_body: Value = test::read_body_json(default_created).await;
    assert_eq!(default_body["is_anonymous"], false);
    assert_eq!(default_body["approval_status"], POST_APPROVED);
    assert_eq!(
        default_body["author_user_id"].as_str(),
        Some(user.id.as_str())
    );
    assert_eq!(default_body["author_name"], "Test User");

    let explicit_created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=false")
            .cookie(access_cookie(&user))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Explicit named body" }))
            .to_request(),
    )
    .await;
    assert_eq!(explicit_created.status(), StatusCode::CREATED);
    let explicit_body: Value = test::read_body_json(explicit_created).await;
    assert_eq!(explicit_body["is_anonymous"], false);
    assert_eq!(
        explicit_body["author_user_id"].as_str(),
        Some(user.id.as_str())
    );
}

#[actix_web::test]
async fn feed_list_returns_has_more_for_paginated_posts() {
    let state = test_state();
    let user = seed_user(
        &state,
        "feed-pagination@example.com",
        false,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state.clone())).await;

    let now = db::now_ts();
    seed_post(&state, &user, "Oldest post", now);
    seed_post(&state, &user, "Middle post", now + 1);
    seed_post(&state, &user, "Newest post", now + 2);

    let first_page = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/feed/posts?page=1&page_size=2")
            .cookie(access_cookie(&user))
            .to_request(),
    )
    .await;
    assert_eq!(first_page.status(), StatusCode::OK);
    let first_body: Value = test::read_body_json(first_page).await;
    assert_eq!(first_body["posts"].as_array().unwrap().len(), 2);
    assert_eq!(first_body["posts"][0]["body"], "Newest post");
    assert_eq!(first_body["posts"][1]["body"], "Middle post");
    assert_eq!(first_body["page"], 1);
    assert_eq!(first_body["page_size"], 2);
    assert_eq!(first_body["has_more"], true);

    let second_page = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/feed/posts?page=2&page_size=2")
            .cookie(access_cookie(&user))
            .to_request(),
    )
    .await;
    assert_eq!(second_page.status(), StatusCode::OK);
    let second_body: Value = test::read_body_json(second_page).await;
    assert_eq!(second_body["posts"].as_array().unwrap().len(), 1);
    assert_eq!(second_body["posts"][0]["body"], "Oldest post");
    assert_eq!(second_body["page"], 2);
    assert_eq!(second_body["page_size"], 2);
    assert_eq!(second_body["has_more"], false);
}

#[actix_web::test]
async fn unified_post_endpoint_rejects_unauthenticated_named_and_invalid_query() {
    let state = test_state();
    let user = seed_user(&state, "query@example.com", false, ACCOUNT_ACTIVE, false);
    let app = test::init_service(configure_app(state)).await;

    let unauthenticated = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=false")
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Named body" }))
            .to_request(),
    )
    .await;
    assert_eq!(unauthenticated.status(), StatusCode::FORBIDDEN);

    let invalid_query = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/feed/posts?anonymous=maybe")
            .cookie(access_cookie(&user))
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "body": "Named body" }))
            .to_request(),
    )
    .await;
    assert_eq!(invalid_query.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn public_survey_response_endpoint_stores_nullable_unowned_response() {
    let state = test_state();
    let app = test::init_service(configure_app(state.clone())).await;

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/survey-responses")
            .peer_addr("127.0.1.10:12345".parse().unwrap())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "food_suggestions": null
            }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(created).await;
    assert!(body["food_suggestions"].is_null());
    assert!(body["dietary_restrictions"].is_null());
    assert!(body["created_at"].as_i64().unwrap() > 0);

    let response = survey_response_row(&state, body["id"].as_str().unwrap());
    assert!(response.food_suggestions.is_none());
    assert!(response.dietary_restrictions.is_none());
}

#[actix_web::test]
async fn public_survey_response_endpoint_truncates_text_to_512_chars() {
    let state = test_state();
    let app = test::init_service(configure_app(state.clone())).await;
    let long_food = "x".repeat(600);
    let long_dietary = "y".repeat(700);

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/survey-responses")
            .peer_addr("127.0.1.11:12345".parse().unwrap())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({
                "food_suggestions": long_food,
                "dietary_restrictions": long_dietary
            }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(created).await;

    let response = survey_response_row(&state, body["id"].as_str().unwrap());
    assert_eq!(response.food_suggestions.unwrap().len(), 512);
    assert_eq!(response.dietary_restrictions.unwrap().len(), 512);
    assert_eq!(body["food_suggestions"].as_str().unwrap().len(), 512);
    assert_eq!(body["dietary_restrictions"].as_str().unwrap().len(), 512);
}

#[actix_web::test]
async fn old_survey_response_paths_are_not_registered() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "old-survey-admin@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state)).await;

    let old_public = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/public/survey-responses")
            .peer_addr("127.0.1.14:12345".parse().unwrap())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "food_suggestions": "chips" }))
            .to_request(),
    )
    .await;
    assert_eq!(old_public.status(), StatusCode::NOT_FOUND);

    let old_top_level_admin_list = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/survey-responses?limit=10")
            .cookie(access_cookie(&admin))
            .to_request(),
    )
    .await;
    assert_eq!(
        old_top_level_admin_list.status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}

#[actix_web::test]
async fn survey_response_endpoint_rate_limits_to_one_per_period() {
    let state = test_state();
    let app = test::init_service(configure_app(state)).await;

    let first = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/survey-responses")
            .peer_addr("127.0.1.15:12345".parse().unwrap())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "food_suggestions": "cookies" }))
            .to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = test::try_call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/survey-responses")
            .peer_addr("127.0.1.15:23456".parse().unwrap())
            .cookie(csrf_cookie())
            .insert_header(("x-xsrf-token", CSRF))
            .set_json(json!({ "food_suggestions": "brownies" }))
            .to_request(),
    )
    .await;
    let status = match second {
        Ok(response) => response.status(),
        Err(error) => error.as_response_error().status_code(),
    };
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[actix_web::test]
async fn admin_survey_response_endpoint_requires_admin() {
    let state = test_state();
    let member = seed_user(
        &state,
        "survey-member@example.com",
        false,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state)).await;

    let unauthenticated = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/survey-responses")
            .to_request(),
    )
    .await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let non_admin = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/survey-responses")
            .cookie(access_cookie(&member))
            .to_request(),
    )
    .await;
    assert_eq!(non_admin.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn admin_survey_response_endpoint_pages_latest_responses() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "survey-list-admin@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    {
        let mut conn = state.db_pool.get().expect("db conn");
        for (id, food, created_at) in [
            ("survey-a", "oldest", 100),
            ("survey-b", "newer b", 200),
            ("survey-c", "newer c", 200),
        ] {
            diesel::insert_into(survey_responses::table)
                .values(NewSurveyResponse {
                    id,
                    food_suggestions: Some(food),
                    dietary_restrictions: None,
                    created_at,
                })
                .execute(&mut conn)
                .expect("insert survey response");
        }
    }
    let app = test::init_service(configure_app(state)).await;

    let first_page = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/survey-responses?page=1&page_size=2")
            .cookie(access_cookie(&admin))
            .to_request(),
    )
    .await;
    assert_eq!(first_page.status(), StatusCode::OK);
    let first_body: Value = test::read_body_json(first_page).await;
    assert_eq!(first_body["page"], 1);
    assert_eq!(first_body["page_size"], 2);
    assert_eq!(first_body["has_more"], true);
    let responses = first_body["responses"].as_array().unwrap();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], "survey-c");
    assert_eq!(responses[1]["id"], "survey-b");

    let second_page = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/survey-responses?page=2&page_size=2")
            .cookie(access_cookie(&admin))
            .to_request(),
    )
    .await;
    assert_eq!(second_page.status(), StatusCode::OK);
    let second_body: Value = test::read_body_json(second_page).await;
    assert_eq!(second_body["page"], 2);
    assert_eq!(second_body["page_size"], 2);
    assert_eq!(second_body["has_more"], false);
    let responses = second_body["responses"].as_array().unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], "survey-a");
}

#[actix_web::test]
async fn admin_survey_response_endpoint_defaults_and_clamps_paging() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "survey-clamp-admin@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    {
        let mut conn = state.db_pool.get().expect("db conn");
        for index in 0..101 {
            let id = Uuid::now_v7().to_string();
            let food = format!("response-{index}");
            diesel::insert_into(survey_responses::table)
                .values(NewSurveyResponse {
                    id: &id,
                    food_suggestions: Some(&food),
                    dietary_restrictions: None,
                    created_at: index,
                })
                .execute(&mut conn)
                .expect("insert survey response");
        }
    }
    let app = test::init_service(configure_app(state)).await;

    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/survey-responses?page=0&page_size=200")
            .cookie(access_cookie(&admin))
            .to_request(),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body: Value = test::read_body_json(listed).await;
    assert_eq!(listed_body["page"], 1);
    assert_eq!(listed_body["page_size"], 100);
    assert_eq!(listed_body["has_more"], true);
    assert_eq!(listed_body["responses"].as_array().unwrap().len(), 100);

    let defaulted = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/survey-responses")
            .cookie(access_cookie(&admin))
            .to_request(),
    )
    .await;
    assert_eq!(defaulted.status(), StatusCode::OK);
    let defaulted_body: Value = test::read_body_json(defaulted).await;
    assert_eq!(defaulted_body["page"], 1);
    assert_eq!(defaulted_body["page_size"], 25);
    assert_eq!(defaulted_body["responses"].as_array().unwrap().len(), 25);
}

#[actix_web::test]
async fn admin_survey_response_endpoint_rejects_old_limit_query() {
    let state = test_state();
    let admin = seed_user(
        &state,
        "survey-limit-admin@example.com",
        true,
        ACCOUNT_ACTIVE,
        false,
    );
    let app = test::init_service(configure_app(state)).await;

    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/admin/survey-responses?limit=10")
            .cookie(access_cookie(&admin))
            .to_request(),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::BAD_REQUEST);
}
