#[allow(dead_code)]
pub fn test_csrf_cookie_and_value() -> (actix_web::cookie::Cookie<'static>, &'static str) {
    const TOKEN: &str = "test-csrf-token";
    let cookie = actix_web::cookie::Cookie::build("xsrf-token", TOKEN)
        .path("/")
        .finish();
    (cookie, TOKEN)
}

pub mod rate_limiting {
    use log::{Level, LevelFilter, Log, Metadata, Record};
    use std::sync::{LazyLock, Mutex, Once};
    use tokio::sync::{Mutex as AsyncMutex, MutexGuard};

    static TEST_LOGGER_INIT: Once = Once::new();
    pub static SHARED_WARNINGS: LazyLock<Mutex<Vec<String>>> =
        LazyLock::new(|| Mutex::new(Vec::new()));
    static SHARED_WARNING_TEST_MUTEX: LazyLock<AsyncMutex<()>> =
        LazyLock::new(|| AsyncMutex::new(()));

    struct SharedTestLogger;

    impl Log for SharedTestLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= Level::Warn
        }

        fn log(&self, record: &Record) {
            if self.enabled(record.metadata()) && record.level() == Level::Warn {
                SHARED_WARNINGS
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(format!("{}", record.args()));
            }
        }

        fn flush(&self) {}
    }

    pub fn init_shared_test_logger() {
        TEST_LOGGER_INIT.call_once(|| {
            let logger = Box::new(SharedTestLogger);
            if log::set_logger(Box::leak(logger)).is_ok() {
                log::set_max_level(LevelFilter::Warn);
            }
        });
        SHARED_WARNINGS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    pub async fn lock_shared_warning_test() -> MutexGuard<'static, ()> {
        SHARED_WARNING_TEST_MUTEX.lock().await
    }
}

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn handlers_and_auth_middleware_do_not_open_db_connections() {
        let files = [
            "src/handlers/admin_handler.rs",
            "src/handlers/auth_handler.rs",
            "src/handlers/feed_handler.rs",
            "src/handlers/notification_handler.rs",
            "src/handlers/public_handler.rs",
            "src/middleware/auth_middleware.rs",
        ];

        for file in files {
            let body = std::fs::read_to_string(file).expect("read source file");
            assert!(
                !body.contains("db_pool.get("),
                "{file} must call services instead of opening DB connections"
            );
            assert!(
                !body.contains("diesel::"),
                "{file} must call services instead of issuing Diesel queries"
            );
            assert!(
                !body.contains("SqliteConnection"),
                "{file} must not accept raw DB connections"
            );
        }
    }
}
