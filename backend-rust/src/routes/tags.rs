use actix_web::{get, web, HttpResponse};

use crate::db::DbPool;
use crate::services::search_service;

#[get("/tags")]
async fn list_tags(
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let tags = search_service::get_all_tags(&mut conn);
    HttpResponse::Ok().json(tags)
}

#[get("/tags/categories")]
async fn list_categories(
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let categories = search_service::get_all_categories(&mut conn);
    HttpResponse::Ok().json(categories)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_tags)
        .service(list_categories);
}
