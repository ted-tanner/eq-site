pub mod api;

use actix_web::web::ServiceConfig;

pub fn configure(cfg: &mut ServiceConfig) {
    api::configure(cfg);
}
