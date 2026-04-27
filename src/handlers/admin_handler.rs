use actix_web::{HttpResponse, web};
use serde::Deserialize;

use crate::AppState;
use crate::handlers::HandlerError;
use crate::middleware::auth_middleware::AuthenticatedUser;
use crate::services::admin::{AdminService, UpsertEventInput, UpsertStudyTopicInput};

#[derive(Debug, Deserialize)]
pub struct PagingQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurveyResponsePagingQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SetAdminRequest {
    pub is_admin: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct UpsertUpcomingEventRequest {
    pub name: String,
    pub event_date: String,
    pub event_time: Option<String>,
    pub end_date: Option<String>,
    pub end_time: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertStudyTopicRequest {
    pub week_start: String,
    pub name: String,
    pub description: Option<String>,
    pub hyperlink: Option<String>,
}

fn event_input(body: &UpsertUpcomingEventRequest) -> UpsertEventInput {
    UpsertEventInput {
        name: body.name.clone(),
        event_date: body.event_date.clone(),
        event_time: body.event_time.clone(),
        end_date: body.end_date.clone(),
        end_time: body.end_time.clone(),
        location: body.location.clone(),
        description: body.description.clone(),
    }
}

fn topic_input(body: &UpsertStudyTopicRequest) -> UpsertStudyTopicInput {
    UpsertStudyTopicInput {
        week_start: body.week_start.clone(),
        name: body.name.clone(),
        description: body.description.clone(),
        hyperlink: body.hyperlink.clone(),
    }
}

pub async fn list_pending(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, HandlerError> {
    let pending_users = AdminService::new(&state.db_pool)
        .list_pending(auth_user.user_id)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "pending_users": pending_users
    })))
}

pub async fn list_pending_anonymous_posts(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, HandlerError> {
    let posts = AdminService::new(&state.db_pool)
        .list_pending_anonymous_posts(auth_user.user_id)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "posts": posts
    })))
}

pub async fn approve_anonymous_post(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    let post = AdminService::new(&state.db_pool)
        .approve_anonymous_post(auth_user.user_id, path.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(post))
}

pub async fn list_events(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, HandlerError> {
    let rows = AdminService::new(&state.db_pool)
        .list_events(auth_user.user_id)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "events": rows })))
}

pub async fn create_event(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    body: web::Json<UpsertUpcomingEventRequest>,
) -> Result<HttpResponse, HandlerError> {
    let created = AdminService::new(&state.db_pool)
        .create_event(auth_user.user_id, event_input(&body))
        .await?;
    Ok(HttpResponse::Created().json(created))
}

pub async fn update_event(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<UpsertUpcomingEventRequest>,
) -> Result<HttpResponse, HandlerError> {
    let updated = AdminService::new(&state.db_pool)
        .update_event(auth_user.user_id, path.into_inner(), event_input(&body))
        .await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn delete_event(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    AdminService::new(&state.db_pool)
        .delete_event(auth_user.user_id, path.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

pub async fn list_study_topics(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, HandlerError> {
    let rows = AdminService::new(&state.db_pool)
        .list_study_topics(auth_user.user_id)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "topics": rows })))
}

pub async fn list_survey_responses(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    query: web::Query<SurveyResponsePagingQuery>,
) -> Result<HttpResponse, HandlerError> {
    let (rows, page, page_size, has_more) = AdminService::new(&state.db_pool)
        .list_survey_responses(
            auth_user.user_id,
            query.page.unwrap_or(1),
            query.page_size.unwrap_or(25),
        )
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "responses": rows,
        "page": page,
        "page_size": page_size,
        "has_more": has_more
    })))
}

pub async fn create_study_topic(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    body: web::Json<UpsertStudyTopicRequest>,
) -> Result<HttpResponse, HandlerError> {
    let created = AdminService::new(&state.db_pool)
        .create_study_topic(auth_user.user_id, topic_input(&body))
        .await?;
    Ok(HttpResponse::Created().json(created))
}

pub async fn update_study_topic(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<UpsertStudyTopicRequest>,
) -> Result<HttpResponse, HandlerError> {
    let updated = AdminService::new(&state.db_pool)
        .update_study_topic(auth_user.user_id, path.into_inner(), topic_input(&body))
        .await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn delete_study_topic(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    AdminService::new(&state.db_pool)
        .delete_study_topic(auth_user.user_id, path.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

pub async fn list_users(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    query: web::Query<PagingQuery>,
) -> Result<HttpResponse, HandlerError> {
    let (users, _, _) = AdminService::new(&state.db_pool)
        .list_users(
            auth_user.user_id,
            query.page.unwrap_or(1),
            query.page_size.unwrap_or(50),
        )
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "users": users })))
}

pub async fn approve_user(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    AdminService::new(&state.db_pool)
        .approve_user(auth_user.user_id, path.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

pub async fn set_admin(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<SetAdminRequest>,
) -> Result<HttpResponse, HandlerError> {
    AdminService::new(&state.db_pool)
        .set_admin(auth_user.user_id, path.into_inner(), body.is_admin)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

pub async fn set_user_status(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<SetStatusRequest>,
) -> Result<HttpResponse, HandlerError> {
    AdminService::new(&state.db_pool)
        .set_user_status(auth_user.user_id, path.into_inner(), body.status.clone())
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

pub async fn reset_password(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    let temp_password = AdminService::new(&state.db_pool)
        .reset_password(auth_user.user_id, path.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "temporary_password": temp_password
    })))
}

pub async fn delete_user(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    AdminService::new(&state.db_pool)
        .delete_user(auth_user.user_id, path.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

pub async fn delete_content(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, HandlerError> {
    let (kind, id) = path.into_inner();
    AdminService::new(&state.db_pool)
        .delete_content(auth_user.user_id, kind, id)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}
