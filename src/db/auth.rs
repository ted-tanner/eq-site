use diesel::SelectableHelper;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::{DaoError, DbPool};
use crate::models::blacklisted_token::NewBlacklistedToken;
use crate::models::user::{NewUser, User};
use crate::schema::{blacklisted_tokens, user_notifications, users};

#[derive(Clone)]
pub struct CreateUserInput {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub account_status: String,
    pub now: i64,
}

#[derive(Clone)]
pub struct ChangePasswordInput {
    pub user_id: String,
    pub current_token_version: i32,
    pub password_hash: String,
    pub must_change_password: bool,
    pub now: i64,
}

pub struct AuthDao {
    db_pool: DbPool,
}

impl AuthDao {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub fn create_user(&self, input: CreateUserInput) -> Result<User, DaoError> {
        let mut conn = self.db_pool.get()?;
        let existing = users::table
            .filter(users::email.eq(&input.email))
            .select(users::id)
            .first::<String>(&mut conn)
            .optional()?;
        if existing.is_some() {
            return Err(DaoError::Requirement("User already exists".to_string()));
        }

        let new_user = NewUser {
            id: &input.id,
            email: &input.email,
            first_name: &input.first_name,
            last_name: &input.last_name,
            password_hash: &input.password_hash,
            token_version: 0,
            is_admin: input.is_admin,
            account_status: &input.account_status,
            must_change_password: false,
            created_at: input.now,
            updated_at: input.now,
        };
        diesel::insert_into(users::table)
            .values(&new_user)
            .execute(&mut conn)?;
        users::table
            .find(&input.id)
            .select(User::as_select())
            .first::<User>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn notify_admins_of_pending_user(&self, pending_user: &User) -> Result<(), DaoError> {
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
                    user_notifications::kind.eq("pending_user"),
                    user_notifications::post_id.eq::<Option<String>>(None),
                    user_notifications::reply_id.eq::<Option<String>>(None),
                    user_notifications::actor_user_id.eq(Some(pending_user.id.clone())),
                    user_notifications::message.eq(format!(
                        "{} {} is awaiting approval",
                        pending_user.first_name, pending_user.last_name
                    )),
                    user_notifications::created_at.eq(crate::db::now_ts()),
                    user_notifications::read_at.eq::<Option<i64>>(None),
                ))
                .execute(&mut conn)?;
        }

        Ok(())
    }

    pub fn blacklist_refresh_signature(
        &self,
        signature_hex: &str,
        expiration: i64,
    ) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::insert_into(blacklisted_tokens::table)
            .values(NewBlacklistedToken {
                token_signature_hex: signature_hex,
                token_expiration: expiration,
            })
            .execute(&mut conn)
            .map(|_| ())
            .map_err(DaoError::from)
    }

    pub fn blacklist_refresh_signature_if_absent(
        &self,
        signature_hex: &str,
        expiration: i64,
    ) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::insert_into(blacklisted_tokens::table)
            .values(NewBlacklistedToken {
                token_signature_hex: signature_hex,
                token_expiration: expiration,
            })
            .execute(&mut conn)
            .ok();
        Ok(())
    }

    pub fn is_refresh_signature_blacklisted(&self, signature_hex: &str) -> Result<bool, DaoError> {
        let mut conn = self.db_pool.get()?;
        blacklisted_tokens::table
            .find(signature_hex)
            .select(blacklisted_tokens::token_signature_hex)
            .first::<String>(&mut conn)
            .optional()
            .map(|row| row.is_some())
            .map_err(DaoError::from)
    }

    pub fn update_password(&self, input: ChangePasswordInput) -> Result<User, DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::update(users::table.find(&input.user_id))
            .set((
                users::password_hash.eq(&input.password_hash),
                users::must_change_password.eq(input.must_change_password),
                users::token_version.eq(input.current_token_version + 1),
                users::updated_at.eq(input.now),
            ))
            .execute(&mut conn)?;
        users::table
            .find(&input.user_id)
            .select(User::as_select())
            .first::<User>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn delete_user(&self, user_id: &str) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::delete(users::table.find(user_id))
            .execute(&mut conn)
            .map(|_| ())
            .map_err(DaoError::from)
    }

    pub fn admin_count(&self) -> Result<i64, DaoError> {
        let mut conn = self.db_pool.get()?;
        users::table
            .filter(users::is_admin.eq(true))
            .count()
            .get_result(&mut conn)
            .map_err(DaoError::from)
    }
}
