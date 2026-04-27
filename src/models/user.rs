use diesel::{Insertable, Queryable, Selectable};
use serde::Serialize;

use crate::schema::users;

#[derive(Clone, Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub token_version: i32,
    pub is_admin: bool,
    pub account_status: String,
    pub must_change_password: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub id: &'a str,
    pub email: &'a str,
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub password_hash: &'a str,
    pub token_version: i32,
    pub is_admin: bool,
    pub account_status: &'a str,
    pub must_change_password: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

pub const ACCOUNT_PENDING: &str = "pending_approval";
pub const ACCOUNT_ACTIVE: &str = "active";
pub const ACCOUNT_SUSPENDED: &str = "suspended";
pub const ACCOUNT_LOCKED: &str = "locked";
