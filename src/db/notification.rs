use diesel::SelectableHelper;
use diesel::prelude::*;

use crate::db::{DaoError, DbPool};
use crate::models::user_notification::UserNotification;
use crate::schema::user_notifications;

pub struct NotificationDao {
    db_pool: DbPool,
}

impl NotificationDao {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub fn list_for_user(
        &self,
        user_id: &str,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<UserNotification>, i64), DaoError> {
        let mut conn = self.db_pool.get()?;
        let notifications = user_notifications::table
            .filter(user_notifications::user_id.eq(user_id))
            .order(user_notifications::created_at.desc())
            .limit(page_size)
            .offset((page - 1) * page_size)
            .select(UserNotification::as_select())
            .load::<UserNotification>(&mut conn)?;
        let unread_count = user_notifications::table
            .filter(user_notifications::user_id.eq(user_id))
            .filter(user_notifications::read_at.is_null())
            .count()
            .get_result(&mut conn)?;
        Ok((notifications, unread_count))
    }

    pub fn mark_read(&self, user_id: &str, ids: &[String], now: i64) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::update(
            user_notifications::table
                .filter(user_notifications::user_id.eq(user_id))
                .filter(user_notifications::id.eq_any(ids)),
        )
        .set(user_notifications::read_at.eq(Some(now)))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn clear_for_user(&self, user_id: &str) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::delete(user_notifications::table.filter(user_notifications::user_id.eq(user_id)))
            .execute(&mut conn)?;
        Ok(())
    }
}
