use async_trait::async_trait;

use crate::db::DbPool;

pub struct JobRunner {
    db_pool: DbPool,
    cleanup_frequency_secs: u64,
}

impl JobRunner {
    pub fn new(db_pool: DbPool, cleanup_frequency_secs: u64) -> Self {
        Self {
            db_pool,
            cleanup_frequency_secs,
        }
    }

    pub async fn start(self) -> ! {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            self.cleanup_frequency_secs.max(1),
        ));
        let job = ClearExpiredBlacklistTokensJob {
            db_pool: self.db_pool.clone(),
        };
        loop {
            interval.tick().await;
            if let Err(error) = job.execute().await {
                log::error!("background cleanup failed: {error}");
            }
        }
    }
}

#[async_trait]
trait Job {
    async fn execute(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

struct ClearExpiredBlacklistTokensJob {
    db_pool: DbPool,
}

#[async_trait]
impl Job for ClearExpiredBlacklistTokensJob {
    async fn execute(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use diesel::prelude::*;

        let pool = self.db_pool.clone();
        tokio::task::spawn_blocking(move || {
            use crate::schema::blacklisted_tokens::dsl::*;
            let mut conn = pool.get()?;
            let now = crate::db::now_ts();
            diesel::delete(blacklisted_tokens.filter(token_expiration.le(now)))
                .execute(&mut conn)?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        })
        .await?
    }
}
