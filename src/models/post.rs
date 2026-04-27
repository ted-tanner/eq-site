use diesel::{Insertable, Queryable, Selectable};
use serde::Serialize;

use crate::schema::posts;

#[derive(Clone, Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = posts)]
pub struct Post {
    pub id: String,
    pub author_user_id: Option<String>,
    pub is_anonymous: bool,
    pub approval_status: String,
    pub body: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Insertable)]
#[diesel(table_name = posts)]
pub struct NewPost<'a> {
    pub id: &'a str,
    pub author_user_id: Option<&'a str>,
    pub is_anonymous: bool,
    pub approval_status: &'a str,
    pub body: &'a str,
    pub created_at: i64,
    pub updated_at: i64,
}

pub const POST_PENDING_APPROVAL: &str = "pending_approval";
pub const POST_APPROVED: &str = "approved";
