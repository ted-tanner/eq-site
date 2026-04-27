use uuid::Uuid;

use crate::db::feed::{CreatePostInput, FeedDao};
use crate::db::user::UserDao;
use crate::db::{self, DbPool};
use crate::models::post::{POST_APPROVED, POST_PENDING_APPROVAL, Post};
use crate::models::reply::Reply;
use crate::models::user::{ACCOUNT_ACTIVE, ACCOUNT_LOCKED, ACCOUNT_PENDING, ACCOUNT_SUSPENDED};
use crate::services::{ServiceError, block_dao};

#[derive(Clone)]
pub struct CreatePostServiceInput {
    pub body: String,
}

#[derive(Clone, Copy)]
pub enum AnonymousPostAuth {
    Unauthenticated,
    ActiveUser,
    PendingUser,
}

pub struct FeedService {
    db_pool: DbPool,
}

impl FeedService {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub async fn list_posts(
        &self,
        user_id: String,
        account_status: &str,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<(Post, Option<String>, i64)>, i64, i64), ServiceError> {
        ensure_token_allows_feed_read(account_status)?;
        let user = self.load_user_and_ensure_read(user_id).await?;
        drop(user);
        let page_size = page_size.clamp(1, 100);
        let page = page.max(1);
        let pool = self.db_pool.clone();
        let rows = block_dao(move || FeedDao::new(&pool).list_posts(page, page_size)).await?;
        let author_ids = rows
            .iter()
            .filter_map(|post| post.author_user_id.clone())
            .collect::<Vec<_>>();
        let post_ids = rows.iter().map(|post| post.id.clone()).collect::<Vec<_>>();
        let pool = self.db_pool.clone();
        let reply_counts =
            block_dao(move || FeedDao::new(&pool).count_replies_by_post(&post_ids)).await?;
        let pool = self.db_pool.clone();
        let author_names =
            block_dao(move || FeedDao::new(&pool).load_author_names(&author_ids)).await?;
        let posts = rows
            .into_iter()
            .map(|post| {
                let author_name = post
                    .author_user_id
                    .clone()
                    .and_then(|id| author_names.get(&id).cloned());
                let reply_count = reply_counts.get(&post.id).copied().unwrap_or(0);
                (post, author_name, reply_count)
            })
            .collect();
        Ok((posts, page, page_size))
    }

    pub async fn get_post_thread(
        &self,
        user_id: String,
        account_status: &str,
        post_id: String,
    ) -> Result<(Post, Option<String>, Vec<(Reply, String)>), ServiceError> {
        ensure_token_allows_feed_read(account_status)?;
        let user = self.load_user_and_ensure_read(user_id).await?;
        drop(user);
        let pool = self.db_pool.clone();
        let (post, replies) = block_dao(move || FeedDao::new(&pool).get_thread(&post_id)).await?;
        let author_ids = replies
            .iter()
            .map(|reply| reply.author_user_id.clone())
            .chain(post.author_user_id.clone())
            .collect::<Vec<_>>();
        let pool = self.db_pool.clone();
        let author_names =
            block_dao(move || FeedDao::new(&pool).load_author_names(&author_ids)).await?;
        let post_author_name = post
            .author_user_id
            .clone()
            .and_then(|id| author_names.get(&id).cloned());
        let replies = replies
            .into_iter()
            .map(|reply| {
                let name = author_names
                    .get(&reply.author_user_id)
                    .cloned()
                    .unwrap_or_else(|| "Unknown User".to_string());
                (reply, name)
            })
            .collect();
        Ok((post, post_author_name, replies))
    }

    pub async fn create_post(
        &self,
        user_id: String,
        input: CreatePostServiceInput,
    ) -> Result<(Post, Option<String>), ServiceError> {
        let user = self.load_user_and_ensure_post(user_id).await?;
        let body = input.body.trim().to_string();
        if body.is_empty() {
            return Err(ServiceError::bad_request("Post body is required"));
        }
        let post_id = Uuid::now_v7().to_string();
        let pool = self.db_pool.clone();
        let created = block_dao(move || {
            FeedDao::new(&pool).create_post(CreatePostInput {
                post_id,
                author: Some(user.clone()),
                body,
                anonymous: false,
                approval_status: POST_APPROVED.to_string(),
                now: db::now_ts(),
            })
        })
        .await?;
        let author_name = self
            .load_user(created.author_user_id.clone().unwrap_or_default())
            .await
            .map(|user| format!("{} {}", user.first_name, user.last_name))?;
        Ok((created, Some(author_name)))
    }

    pub async fn create_anonymous_post(
        &self,
        auth: AnonymousPostAuth,
        input: CreatePostServiceInput,
    ) -> Result<(Post, Option<String>), ServiceError> {
        let body = input.body.trim().to_string();
        if body.is_empty() {
            return Err(ServiceError::bad_request("Post body is required"));
        }
        let approval_status = match auth {
            AnonymousPostAuth::Unauthenticated | AnonymousPostAuth::PendingUser => {
                POST_PENDING_APPROVAL
            }
            AnonymousPostAuth::ActiveUser => POST_APPROVED,
        };
        let post_id = Uuid::now_v7().to_string();
        let pool = self.db_pool.clone();
        let created = block_dao(move || {
            FeedDao::new(&pool).create_post(CreatePostInput {
                post_id,
                author: None,
                body,
                anonymous: true,
                approval_status: approval_status.to_string(),
                now: db::now_ts(),
            })
        })
        .await?;
        if created.approval_status == POST_PENDING_APPROVAL {
            let pool = self.db_pool.clone();
            let pending_post = created.clone();
            block_dao(move || {
                FeedDao::new(&pool).notify_admins_of_pending_anonymous_post(&pending_post)
            })
            .await?;
        }
        Ok((created, None))
    }

    pub async fn create_reply(
        &self,
        user_id: String,
        post_id: String,
        body: String,
    ) -> Result<(Reply, String), ServiceError> {
        let user = self.load_user_and_ensure_post(user_id).await?;
        let body = body.trim().to_string();
        if body.is_empty() {
            return Err(ServiceError::bad_request("Reply body is required"));
        }
        let author_name = format!("{} {}", user.first_name, user.last_name);
        let reply_id = Uuid::now_v7().to_string();
        let pool = self.db_pool.clone();
        let reply = block_dao(move || {
            FeedDao::new(&pool).create_reply(&post_id, &reply_id, user, &body, db::now_ts())
        })
        .await?;
        Ok((reply, author_name))
    }

    pub async fn delete_post(&self, user_id: String, post_id: String) -> Result<(), ServiceError> {
        let user = self.load_user(user_id).await?;
        let pool = self.db_pool.clone();
        let post_id_for_load = post_id.clone();
        let post = block_dao(move || FeedDao::new(&pool).load_post(&post_id_for_load)).await?;
        if !user.is_admin && post.author_user_id.as_deref() != Some(user.id.as_str()) {
            return Err(ServiceError::forbidden(
                "You can only delete your own posts",
            ));
        }
        let pool = self.db_pool.clone();
        block_dao(move || FeedDao::new(&pool).delete_post(&post_id)).await
    }

    pub async fn delete_reply(
        &self,
        user_id: String,
        reply_id: String,
    ) -> Result<(), ServiceError> {
        let user = self.load_user(user_id).await?;
        let pool = self.db_pool.clone();
        let reply_id_for_load = reply_id.clone();
        let reply = block_dao(move || FeedDao::new(&pool).load_reply(&reply_id_for_load)).await?;
        if !user.is_admin && reply.author_user_id != user.id {
            return Err(ServiceError::forbidden(
                "You can only delete your own replies",
            ));
        }
        let pool = self.db_pool.clone();
        block_dao(move || FeedDao::new(&pool).delete_reply(&reply_id)).await
    }

    async fn load_user(&self, user_id: String) -> Result<crate::models::user::User, ServiceError> {
        let pool = self.db_pool.clone();
        block_dao(move || UserDao::new(&pool).load_user(&user_id)).await
    }

    async fn load_user_and_ensure_read(
        &self,
        user_id: String,
    ) -> Result<crate::models::user::User, ServiceError> {
        let user = self.load_user(user_id).await?;
        match user.account_status.as_str() {
            ACCOUNT_ACTIVE | ACCOUNT_SUSPENDED => Ok(user),
            ACCOUNT_PENDING => Err(ServiceError::forbidden(
                "Your account is awaiting approval from a member of the EQ presidency",
            )),
            ACCOUNT_LOCKED => Err(ServiceError::forbidden(
                "Your account has been locked by a member of the EQ presidency",
            )),
            _ => Err(ServiceError::forbidden(
                "Account is not allowed to access the feed",
            )),
        }
    }

    async fn load_user_and_ensure_post(
        &self,
        user_id: String,
    ) -> Result<crate::models::user::User, ServiceError> {
        let user = self.load_user(user_id).await?;
        match user.account_status.as_str() {
            ACCOUNT_ACTIVE => Ok(user),
            ACCOUNT_SUSPENDED => Err(ServiceError::forbidden(
                "Your account has been suspended from posting and replying by a member of the EQ presidency",
            )),
            ACCOUNT_PENDING => Err(ServiceError::forbidden(
                "Your account is awaiting approval from a member of the EQ presidency",
            )),
            ACCOUNT_LOCKED => Err(ServiceError::forbidden(
                "Your account has been locked by a member of the EQ presidency",
            )),
            _ => Err(ServiceError::forbidden("Account is not allowed to post")),
        }
    }
}

fn ensure_token_allows_feed_read(account_status: &str) -> Result<(), ServiceError> {
    match account_status {
        ACCOUNT_ACTIVE | ACCOUNT_SUSPENDED => Ok(()),
        ACCOUNT_PENDING => Err(ServiceError::forbidden(
            "Your account is awaiting approval from a member of the EQ presidency",
        )),
        ACCOUNT_LOCKED => Err(ServiceError::forbidden(
            "Your account has been locked by a member of the EQ presidency",
        )),
        _ => Err(ServiceError::forbidden(
            "Account is not allowed to access the feed",
        )),
    }
}
