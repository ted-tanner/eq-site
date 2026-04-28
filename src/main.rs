use actix_files::NamedFile;
use actix_web::{App, HttpServer, middleware::DefaultHeaders, middleware::Logger, web};
use flexi_logger::{Age, Cleanup, Criterion, Duplicate, FileSpec, Naming, WriteMode};
use rustls::ServerConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

mod auth;
mod db;
mod env;
mod handlers;
#[cfg(test)]
mod integration_tests;
mod jobs;
mod middleware;
mod models;
mod routes;
mod schema;
mod services;
#[cfg(test)]
mod test_utils;
mod utils;

use db::create_db_pool;
pub struct AppState {
    pub db_pool: db::DbPool,
}

pub fn configure_app(
    state: web::Data<AppState>,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(state)
        .wrap(
            DefaultHeaders::new()
                .add(("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"))
                .add(("X-Content-Type-Options", "nosniff"))
                .add(("Referrer-Policy", "same-origin"))
                .add(("Permissions-Policy", "camera=(), microphone=(), geolocation=(), payments=()"))
                .add(("X-Frame-Options", "DENY")),
        )
        .wrap(middleware::cors_middleware::CorsMiddleware::default())
        .wrap(Logger::default())
        .configure(routes::configure)
        .route("/survey", web::get().to(survey_entrypoint))
        .service(actix_files::Files::new("/", env::CONF.web_path.clone()).index_file("index.html"))
}

async fn survey_entrypoint() -> actix_web::Result<NamedFile> {
    NamedFile::open_async(env::CONF.web_path.join("index.html"))
        .await
        .map_err(Into::into)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let log_dir = env::CONF
        .log_path
        .as_deref()
        .unwrap_or(std::path::Path::new("./logs"));

    let _logger = flexi_logger::Logger::try_with_str(&env::CONF.log_level)
        .expect("invalid log level")
        .log_to_file(FileSpec::default().directory(log_dir))
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(30),
        )
        .cleanup_in_background_thread(true)
        .duplicate_to_stdout(Duplicate::All)
        .write_mode(WriteMode::BufferAndFlush)
        .format(|writer, now, record| {
            write!(
                writer,
                "{:5} | {} | {}:{} | {}",
                record.level(),
                now.format("%Y-%m-%dT%H:%M:%S%.6fZ"),
                record.module_path().unwrap_or("<unknown>"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .use_utc()
        .start()
        .expect("start logger");

    let db_pool = create_db_pool(&env::CONF.database_url, env::CONF.db_max_connections);
    let mut conn = db_pool.get().expect("db conn");
    db::run_migrations(&mut conn).expect("run migrations");
    drop(conn);

    if env::CONF.run_background_tasks {
        let runner = jobs::JobRunner::new(
            db_pool.clone(),
            env::CONF.blacklist_cleanup_job_frequency.as_secs(),
        );
        tokio::spawn(runner.start());
    }

    let state = web::Data::new(AppState { db_pool });

    let addr = format!("{}:{}", env::CONF.bind_host, env::CONF.port);
    let server =
        HttpServer::new(move || configure_app(state.clone())).workers(env::CONF.actix_worker_count);

    match (&env::CONF.tls_cert_path, &env::CONF.tls_key_path) {
        (Some(cert), Some(key)) => {
            let tls_config = load_rustls_config(cert, key)?;
            server.bind_rustls_0_23(addr, tls_config)?.run().await
        }
        _ => server.bind(addr)?.run().await,
    }
}

fn load_rustls_config(
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
) -> std::io::Result<ServerConfig> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;

    let cert_chain: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        .filter_map(Result::ok)
        .map(|cert| cert.into_owned())
        .collect();
    let key_der = PrivateKeyDer::from_pem_file(key_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        .clone_key();

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key_der)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
