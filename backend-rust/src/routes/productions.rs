use actix_web::{get, post, put, delete, web, HttpResponse};

use crate::db::DbPool;
use crate::models::ProductionCreate;
use crate::services::production_service;

#[get("/productions")]
async fn list_productions(
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let productions = production_service::get_all_productions(&mut conn);
    HttpResponse::Ok().json(productions)
}

#[post("/productions")]
async fn create_production(
    pool: web::Data<DbPool>,
    body: web::Json<ProductionCreate>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");

    match production_service::create_production(&mut conn, &body) {
        Ok(prod) => HttpResponse::Created().json(prod),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "detail": e,
        })),
    }
}

#[put("/productions/{production_id}")]
async fn update_production(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
    body: web::Json<ProductionCreate>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let production_id = path.into_inner();

    match production_service::update_production(&mut conn, production_id, &body) {
        Some(prod) => HttpResponse::Ok().json(prod),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Production not found: {}", production_id),
        })),
    }
}

#[delete("/productions/{production_id}")]
async fn delete_production(
    pool: web::Data<DbPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let mut conn = pool.get().expect("Failed to get DB connection");
    let production_id = path.into_inner();

    if production_service::delete_production(&mut conn, production_id) {
        HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Production {} deleted", production_id),
        }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "detail": format!("Production not found: {}", production_id),
        }))
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_productions)
        .service(create_production)
        .service(update_production)
        .service(delete_production);
}
