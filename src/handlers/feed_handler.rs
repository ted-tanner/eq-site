use actix_web::{HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::handlers::HandlerError;
use crate::middleware::auth_middleware::AuthenticatedUser;
use crate::models::post::Post;
use crate::models::reply::Reply;
use crate::models::user::{ACCOUNT_ACTIVE, ACCOUNT_LOCKED, ACCOUNT_PENDING, ACCOUNT_SUSPENDED};
use crate::services::auth::AuthService;
use crate::services::feed::{AnonymousPostAuth, CreatePostServiceInput, FeedService};

#[derive(Debug, Deserialize)]
pub struct PagingQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePostRequest {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePostQuery {
    pub anonymous: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReplyRequest {
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct FeedPostResponse {
    pub id: String,
    pub body: String,
    pub is_anonymous: bool,
    pub approval_status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub author_user_id: Option<String>,
    pub author_name: Option<String>,
    pub reply_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ReplyResponse {
    pub id: String,
    pub post_id: String,
    pub body: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub author_name: String,
    pub author_user_id: String,
}

#[derive(Debug, Serialize)]
pub struct ThreadResponse {
    pub post: FeedPostResponse,
    pub replies: Vec<ReplyResponse>,
}

pub(crate) fn post_to_response(
    post: Post,
    author_name: Option<String>,
    reply_count: i64,
) -> FeedPostResponse {
    FeedPostResponse {
        id: post.id,
        body: post.body,
        is_anonymous: post.is_anonymous,
        approval_status: post.approval_status,
        created_at: post.created_at,
        updated_at: post.updated_at,
        author_user_id: post.author_user_id,
        author_name,
        reply_count,
    }
}

fn reply_to_response(reply: Reply, author_name: String) -> ReplyResponse {
    ReplyResponse {
        id: reply.id,
        post_id: reply.post_id,
        body: reply.body,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
        author_name,
        author_user_id: reply.author_user_id,
    }
}

pub async fn list_posts(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    query: web::Query<PagingQuery>,
) -> Result<HttpResponse, HandlerError> {
    let (posts, page, page_size, has_more) = FeedService::new(&state.db_pool)
        .list_posts(
            auth_user.user_id,
            &auth_user.account_status,
            query.page.unwrap_or(1),
            query.page_size.unwrap_or(20),
        )
        .await?;
    let response = posts
        .into_iter()
        .map(|(post, author_name, reply_count)| post_to_response(post, author_name, reply_count))
        .collect::<Vec<_>>();
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "posts": response,
        "page": page,
        "page_size": page_size,
        "has_more": has_more
    })))
}

pub async fn get_post_thread(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    let (post, post_author_name, replies) = FeedService::new(&state.db_pool)
        .get_post_thread(
            auth_user.user_id,
            &auth_user.account_status,
            path.into_inner(),
        )
        .await?;
    let reply_count = replies.len() as i64;
    Ok(HttpResponse::Ok().json(ThreadResponse {
        post: post_to_response(post, post_author_name, reply_count),
        replies: replies
            .into_iter()
            .map(|(reply, author_name)| reply_to_response(reply, author_name))
            .collect(),
    }))
}

pub async fn create_post(
    state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<CreatePostQuery>,
    body: web::Json<CreatePostRequest>,
) -> Result<HttpResponse, HandlerError> {
    let input = CreatePostServiceInput {
        body: body.body.clone(),
    };
    let service = FeedService::new(&state.db_pool);
    let (post, author_name) = if query.anonymous.unwrap_or(false) {
        let auth = anonymous_post_auth(&state, &req).await?;
        service.create_anonymous_post(auth, input).await?
    } else {
        let user_id = required_post_user_id(&state, &req).await?;
        service.create_post(user_id, input).await?
    };
    Ok(HttpResponse::Created().json(post_to_response(post, author_name, 0)))
}

async fn required_post_user_id(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> Result<String, HandlerError> {
    let Some(token) = req
        .cookie("access_token")
        .map(|cookie| cookie.value().to_string())
    else {
        return Err(HandlerError::Forbidden(
            "Authentication is required to create a post".into(),
        ));
    };

    let session = AuthService::new(&state.db_pool)
        .validate_access_token(token)
        .await
        .map_err(|_| HandlerError::unauthorized("Invalid access token"))?;
    Ok(session.user_id)
}

async fn anonymous_post_auth(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> Result<AnonymousPostAuth, HandlerError> {
    let Some(token) = req
        .cookie("access_token")
        .map(|cookie| cookie.value().to_string())
    else {
        return Ok(AnonymousPostAuth::Unauthenticated);
    };

    let auth_service = AuthService::new(&state.db_pool);
    let session = auth_service
        .validate_access_token(token)
        .await
        .map_err(|_| HandlerError::unauthorized("Invalid access token"))?;
    let user = auth_service.load_user(session.user_id).await?;
    match user.account_status.as_str() {
        ACCOUNT_ACTIVE => Ok(AnonymousPostAuth::ActiveUser),
        ACCOUNT_PENDING => Ok(AnonymousPostAuth::PendingUser),
        ACCOUNT_SUSPENDED => Err(HandlerError::Forbidden(
            "Your account has been suspended from posting and replying by a member of the EQ presidency"
                .into(),
        )),
        ACCOUNT_LOCKED => Err(HandlerError::Forbidden(
            "Your account has been locked by a member of the EQ presidency".into(),
        )),
        _ => Err(HandlerError::Forbidden(
            "Account is not allowed to post".into(),
        )),
    }
}

pub async fn create_reply(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<CreateReplyRequest>,
) -> Result<HttpResponse, HandlerError> {
    let (reply, author_name) = FeedService::new(&state.db_pool)
        .create_reply(auth_user.user_id, path.into_inner(), body.body.clone())
        .await?;
    Ok(HttpResponse::Created().json(reply_to_response(reply, author_name)))
}

pub async fn delete_post(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    FeedService::new(&state.db_pool)
        .delete_post(auth_user.user_id, path.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

pub async fn delete_reply(
    state: web::Data<AppState>,
    auth_user: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, HandlerError> {
    FeedService::new(&state.db_pool)
        .delete_reply(auth_user.user_id, path.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}
