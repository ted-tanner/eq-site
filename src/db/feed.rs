use std::collections::{HashMap, HashSet};

use diesel::SelectableHelper;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::{DaoError, DbPool};
use crate::models::post::{NewPost, POST_APPROVED, Post};
use crate::models::reply::{NewReply, Reply};
use crate::models::user::User;
use crate::schema::{posts, replies, user_notifications, users};

#[derive(Clone)]
pub struct CreatePostInput {
    pub post_id: String,
    pub author: Option<User>,
    pub body: String,
    pub anonymous: bool,
    pub approval_status: String,
    pub now: i64,
}

pub struct FeedDao {
    db_pool: DbPool,
}

impl FeedDao {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub fn load_author_names(
        &self,
        user_ids: &[String],
    ) -> Result<HashMap<String, String>, DaoError> {
        if user_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut conn = self.db_pool.get()?;
        let rows: Vec<(String, String, String)> = users::table
            .filter(users::id.eq_any(user_ids))
            .select((users::id, users::first_name, users::last_name))
            .load(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(|(id, first_name, last_name)| (id, format!("{first_name} {last_name}")))
            .collect())
    }

    pub fn list_posts(&self, page: i64, page_size: i64) -> Result<Vec<Post>, DaoError> {
        let mut conn = self.db_pool.get()?;
        posts::table
            .filter(posts::approval_status.eq(POST_APPROVED))
            .order(posts::created_at.desc())
            .then_order_by(posts::id.desc())
            .limit(page_size)
            .offset((page - 1) * page_size)
            .select(Post::as_select())
            .load::<Post>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn get_thread(&self, post_id: &str) -> Result<(Post, Vec<Reply>), DaoError> {
        let mut conn = self.db_pool.get()?;
        let post = posts::table
            .find(post_id)
            .filter(posts::approval_status.eq(POST_APPROVED))
            .select(Post::as_select())
            .first::<Post>(&mut conn)?;
        let reply_rows = replies::table
            .filter(replies::post_id.eq(post_id))
            .order(replies::created_at.asc())
            .select(Reply::as_select())
            .load::<Reply>(&mut conn)?;
        Ok((post, reply_rows))
    }

    pub fn create_post(&self, input: CreatePostInput) -> Result<Post, DaoError> {
        let mut conn = self.db_pool.get()?;
        let author_id = input.author.as_ref().map(|author| author.id.as_str());
        diesel::insert_into(posts::table)
            .values(&NewPost {
                id: &input.post_id,
                author_user_id: author_id,
                is_anonymous: input.anonymous,
                approval_status: &input.approval_status,
                body: &input.body,
                created_at: input.now,
                updated_at: input.now,
            })
            .execute(&mut conn)?;
        posts::table
            .find(&input.post_id)
            .select(Post::as_select())
            .first::<Post>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn notify_admins_of_pending_anonymous_post(&self, post: &Post) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        let admin_ids: Vec<String> = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .load(&mut conn)?;

        for admin_id in admin_ids {
            diesel::insert_into(user_notifications::table)
                .values((
                    user_notifications::id.eq(Uuid::now_v7().to_string()),
                    user_notifications::user_id.eq(admin_id),
                    user_notifications::kind.eq("pending_anonymous_post"),
                    user_notifications::post_id.eq(Some(post.id.clone())),
                    user_notifications::reply_id.eq::<Option<String>>(None),
                    user_notifications::actor_user_id.eq::<Option<String>>(None),
                    user_notifications::message.eq("An anonymous post is awaiting approval"),
                    user_notifications::created_at.eq(post.created_at),
                    user_notifications::read_at.eq::<Option<i64>>(None),
                ))
                .execute(&mut conn)?;
        }

        Ok(())
    }

    pub fn create_reply(
        &self,
        post_id: &str,
        reply_id: &str,
        user: User,
        body: &str,
        now: i64,
    ) -> Result<Reply, DaoError> {
        let mut conn = self.db_pool.get()?;
        conn.transaction(|conn| {
            let post = posts::table
                .find(post_id)
                .filter(posts::approval_status.eq(POST_APPROVED))
                .select(Post::as_select())
                .first::<Post>(conn)?;
            diesel::insert_into(replies::table)
                .values(NewReply {
                    id: reply_id,
                    post_id,
                    author_user_id: &user.id,
                    body,
                    created_at: now,
                    updated_at: now,
                })
                .execute(conn)?;
            if !post.is_anonymous {
                let mut notified = HashSet::new();
                if let Some(author_id) = post.author_user_id.as_ref()
                    && author_id != &user.id
                {
                    notified.insert(author_id.clone());
                }
                let prior_repliers: Vec<String> = replies::table
                    .filter(replies::post_id.eq(post_id))
                    .filter(replies::author_user_id.ne(&user.id))
                    .select(replies::author_user_id)
                    .load(conn)?;
                for user_id in prior_repliers {
                    notified.insert(user_id);
                }
                for notify_user_id in notified {
                    diesel::insert_into(user_notifications::table)
                        .values((
                            user_notifications::id.eq(Uuid::now_v7().to_string()),
                            user_notifications::user_id.eq(notify_user_id),
                            user_notifications::kind.eq("reply"),
                            user_notifications::post_id.eq(Some(post_id.to_string())),
                            user_notifications::reply_id.eq(Some(reply_id.to_string())),
                            user_notifications::actor_user_id.eq(Some(user.id.clone())),
                            user_notifications::message.eq(format!(
                                "{} {} replied to a post you are following",
                                user.first_name, user.last_name
                            )),
                            user_notifications::created_at.eq(now),
                            user_notifications::read_at.eq::<Option<i64>>(None),
                        ))
                        .execute(conn)?;
                }
            }
            replies::table
                .find(reply_id)
                .select(Reply::as_select())
                .first::<Reply>(conn)
        })
        .map_err(DaoError::from)
    }

    pub fn load_post(&self, post_id: &str) -> Result<Post, DaoError> {
        let mut conn = self.db_pool.get()?;
        posts::table
            .find(post_id)
            .select(Post::as_select())
            .first::<Post>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn load_reply(&self, reply_id: &str) -> Result<Reply, DaoError> {
        let mut conn = self.db_pool.get()?;
        replies::table
            .find(reply_id)
            .select(Reply::as_select())
            .first::<Reply>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn delete_post(&self, post_id: &str) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::delete(posts::table.find(post_id))
            .execute(&mut conn)
            .map(|_| ())
            .map_err(DaoError::from)
    }

    pub fn delete_reply(&self, reply_id: &str) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::delete(replies::table.find(reply_id))
            .execute(&mut conn)
            .map(|_| ())
            .map_err(DaoError::from)
    }
}
