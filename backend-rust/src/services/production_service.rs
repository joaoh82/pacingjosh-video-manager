use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::models::*;
use crate::schema::{productions, video_productions};

/// Get all productions with video counts
pub fn get_all_productions(conn: &mut SqliteConnection) -> Vec<ProductionResponse> {
    let results: Vec<(Production, Option<i64>)> = productions::table
        .left_join(video_productions::table)
        .group_by(productions::id)
        .select((
            productions::all_columns,
            diesel::dsl::sql::<diesel::sql_types::Nullable<diesel::sql_types::BigInt>>(
                "COUNT(video_productions.video_id)"
            ),
        ))
        .order(productions::title.asc())
        .load(conn)
        .unwrap_or_default();

    results.into_iter().map(|(p, count)| {
        ProductionResponse {
            id: p.id,
            title: p.title,
            platform: p.platform,
            link: p.link,
            is_published: p.is_published,
            video_count: count.unwrap_or(0),
        }
    }).collect()
}

/// Get a single production by ID
pub fn get_production(conn: &mut SqliteConnection, production_id: i32) -> Option<Production> {
    productions::table.find(production_id).first(conn).ok()
}

/// Get a production response with video count
pub fn get_production_response(conn: &mut SqliteConnection, production_id: i32) -> Option<ProductionResponse> {
    let prod = get_production(conn, production_id)?;

    let video_count: i64 = video_productions::table
        .filter(video_productions::production_id.eq(production_id))
        .count()
        .get_result(conn)
        .unwrap_or(0);

    Some(ProductionResponse {
        id: prod.id,
        title: prod.title,
        platform: prod.platform,
        link: prod.link,
        is_published: prod.is_published,
        video_count,
    })
}

/// Create a new production
pub fn create_production(
    conn: &mut SqliteConnection,
    data: &ProductionCreate,
) -> Result<ProductionResponse, String> {
    // Check uniqueness
    let exists = productions::table
        .filter(productions::title.eq(&data.title))
        .count()
        .get_result::<i64>(conn)
        .unwrap_or(0) > 0;

    if exists {
        return Err(format!("Production with title '{}' already exists", data.title));
    }

    diesel::insert_into(productions::table)
        .values(&NewProduction {
            title: data.title.clone(),
            platform: data.platform.clone(),
            link: data.link.clone(),
            is_published: data.is_published,
        })
        .execute(conn)
        .map_err(|e| e.to_string())?;

    // Get the created production
    let prod: Production = productions::table
        .filter(productions::title.eq(&data.title))
        .first(conn)
        .map_err(|e| e.to_string())?;

    Ok(ProductionResponse {
        id: prod.id,
        title: prod.title,
        platform: prod.platform,
        link: prod.link,
        is_published: prod.is_published,
        video_count: 0,
    })
}

/// Update an existing production
pub fn update_production(
    conn: &mut SqliteConnection,
    production_id: i32,
    data: &ProductionCreate,
) -> Option<ProductionResponse> {
    let _prod = get_production(conn, production_id)?;

    diesel::update(productions::table.find(production_id))
        .set(&ProductionChangeset {
            title: data.title.clone(),
            platform: data.platform.clone(),
            link: data.link.clone(),
            is_published: data.is_published,
        })
        .execute(conn)
        .ok()?;

    get_production_response(conn, production_id)
}

/// Delete a production
pub fn delete_production(conn: &mut SqliteConnection, production_id: i32) -> bool {
    let result = diesel::delete(productions::table.find(production_id))
        .execute(conn);

    matches!(result, Ok(count) if count > 0)
}
