use actix_web::{HttpResponse, web};
use serde::Deserialize;

use crate::AppState;
use crate::handlers::HandlerError;
use crate::middleware::auth_middleware::AuthenticatedUser;
use crate::services::notification::NotificationService;

#[derive(Debug, Deserialize)]
pub struct PagingQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MarkReadRequest {
    pub ids: Vec<String>,
}

pub async fn list_notifications(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    query: web::Query<PagingQuery>,
) -> Result<HttpResponse, HandlerError> {
    let (notifications, unread_count, page, page_size) = NotificationService::new(&state.db_pool)
        .list_notifications(
            auth_user.user_id,
            query.page.unwrap_or(1),
            query.page_size.unwrap_or(20),
        )
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "notifications": notifications,
        "unread_count": unread_count,
        "page": page,
        "page_size": page_size
    })))
}

pub async fn mark_notifications_read(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    body: web::Json<MarkReadRequest>,
) -> Result<HttpResponse, HandlerError> {
    NotificationService::new(&state.db_pool)
        .mark_notifications_read(auth_user.user_id, body.ids.clone())
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

pub async fn clear_notifications(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
) -> Result<HttpResponse, HandlerError> {
    NotificationService::new(&state.db_pool)
        .clear_notifications(auth_user.user_id)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}
