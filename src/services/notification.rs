use crate::db::notification::NotificationDao;
use crate::db::user::UserDao;
use crate::db::{self, DbPool};
use crate::models::user_notification::UserNotification;
use crate::services::{ServiceError, block_dao};

pub struct NotificationService {
    db_pool: DbPool,
}

impl NotificationService {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub async fn list_notifications(
        &self,
        user_id: String,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<UserNotification>, i64, i64, i64), ServiceError> {
        let page_size = page_size.clamp(1, 100);
        let page = page.max(1);
        let pool = self.db_pool.clone();
        let user = block_dao(move || UserDao::new(&pool).load_user(&user_id)).await?;
        let pool = self.db_pool.clone();
        let (notifications, unread_count) =
            block_dao(move || NotificationDao::new(&pool).list_for_user(&user.id, page, page_size))
                .await?;
        Ok((notifications, unread_count, page, page_size))
    }

    pub async fn mark_notifications_read(
        &self,
        user_id: String,
        ids: Vec<String>,
    ) -> Result<(), ServiceError> {
        let pool = self.db_pool.clone();
        let user = block_dao(move || UserDao::new(&pool).load_user(&user_id)).await?;
        let pool = self.db_pool.clone();
        block_dao(move || NotificationDao::new(&pool).mark_read(&user.id, &ids, db::now_ts())).await
    }

    pub async fn clear_notifications(&self, user_id: String) -> Result<(), ServiceError> {
        let pool = self.db_pool.clone();
        let user = block_dao(move || UserDao::new(&pool).load_user(&user_id)).await?;
        let pool = self.db_pool.clone();
        block_dao(move || NotificationDao::new(&pool).clear_for_user(&user.id)).await
    }
}
