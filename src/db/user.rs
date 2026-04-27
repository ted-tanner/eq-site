use diesel::SelectableHelper;
use diesel::prelude::*;

use crate::db::{DaoError, DbPool};
use crate::models::user::User;
use crate::schema::users;

pub struct UserDao {
    db_pool: DbPool,
}

impl UserDao {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub fn load_user(&self, user_id: &str) -> Result<User, DaoError> {
        let mut conn = self.db_pool.get()?;
        users::table
            .find(user_id)
            .select(User::as_select())
            .first::<User>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn load_user_by_email(&self, email: &str) -> Result<User, DaoError> {
        let mut conn = self.db_pool.get()?;
        users::table
            .filter(users::email.eq(email))
            .select(User::as_select())
            .first::<User>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn load_session_user(&self, user_id: &str, token_version: i32) -> Result<User, DaoError> {
        let user = self.load_user(user_id)?;
        if user.token_version == token_version {
            Ok(user)
        } else {
            Err(DaoError::NotFound)
        }
    }
}
