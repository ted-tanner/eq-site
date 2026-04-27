use diesel::{Insertable, Queryable, Selectable};
use serde::Serialize;

use crate::schema::replies;

#[derive(Clone, Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = replies)]
pub struct Reply {
    pub id: String,
    pub post_id: String,
    pub author_user_id: String,
    pub body: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Insertable)]
#[diesel(table_name = replies)]
pub struct NewReply<'a> {
    pub id: &'a str,
    pub post_id: &'a str,
    pub author_user_id: &'a str,
    pub body: &'a str,
    pub created_at: i64,
    pub updated_at: i64,
}
