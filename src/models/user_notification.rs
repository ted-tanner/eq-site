use diesel::{Insertable, Queryable, Selectable};
use serde::Serialize;

use crate::schema::user_notifications;

#[derive(Clone, Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = user_notifications)]
pub struct UserNotification {
    pub id: String,
    pub user_id: String,
    pub kind: String,
    pub post_id: Option<String>,
    pub reply_id: Option<String>,
    pub actor_user_id: Option<String>,
    pub message: String,
    pub created_at: i64,
    pub read_at: Option<i64>,
}

#[derive(Insertable)]
#[diesel(table_name = user_notifications)]
pub struct NewUserNotification<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub kind: &'a str,
    pub post_id: Option<&'a str>,
    pub reply_id: Option<&'a str>,
    pub actor_user_id: Option<&'a str>,
    pub message: &'a str,
    pub created_at: i64,
    pub read_at: Option<i64>,
}
