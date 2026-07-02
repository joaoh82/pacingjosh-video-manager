use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::models::*;
use crate::schema::{productions, video_productions, videos};

/// Parse a publish date sent by the client — either a bare date ("2026-07-02",
/// from an `<input type="date">`) or a full ISO datetime. Blank → None.
fn parse_published_at(raw: Option<&str>) -> Option<NaiveDateTime> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
        .ok()
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        })
}

/// The publish date to persist: an explicit date always wins; publishing
/// without one falls back to the previously saved date, then to "now".
fn resolve_published_at(
    explicit: Option<NaiveDateTime>,
    existing: Option<NaiveDateTime>,
    is_published: bool,
) -> Option<NaiveDateTime> {
    if is_published {
        explicit
            .or(existing)
            .or_else(|| Some(chrono::Utc::now().naive_utc()))
    } else {
        explicit
    }
}

/// Load all videos linked to a production, ordered by creation date then id so
/// the edit pipeline sees takes in roughly the order they were recorded.
pub fn get_production_videos(conn: &mut SqliteConnection, production_id: i32) -> Vec<Video> {
    videos::table
        .inner_join(video_productions::table.on(video_productions::video_id.eq(videos::id)))
        .filter(video_productions::production_id.eq(production_id))
        .select(Video::as_select())
        .order((videos::created_date.asc(), videos::id.asc()))
        .load(conn)
        .unwrap_or_default()
}

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
            production_type: p.production_type,
            published_at: p.published_at,
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
        production_type: prod.production_type,
        published_at: prod.published_at,
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
            production_type: normalize_production_type(&data.production_type),
            published_at: resolve_published_at(
                parse_published_at(data.published_at.as_deref()),
                None,
                data.is_published,
            ),
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
        production_type: prod.production_type,
        published_at: prod.published_at,
        video_count: 0,
    })
}

/// Update an existing production
pub fn update_production(
    conn: &mut SqliteConnection,
    production_id: i32,
    data: &ProductionCreate,
) -> Option<ProductionResponse> {
    let prod = get_production(conn, production_id)?;

    diesel::update(productions::table.find(production_id))
        .set(&ProductionChangeset {
            title: data.title.clone(),
            platform: data.platform.clone(),
            link: data.link.clone(),
            is_published: data.is_published,
            production_type: normalize_production_type(&data.production_type),
            published_at: resolve_published_at(
                parse_published_at(data.published_at.as_deref()),
                prod.published_at,
                data.is_published,
            ),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    #[test]
    fn parse_accepts_bare_date_and_iso_datetime() {
        assert_eq!(parse_published_at(Some("2026-07-02")), Some(dt("2026-07-02")));
        assert_eq!(
            parse_published_at(Some("2026-07-02T15:30:00")),
            Some(dt("2026-07-02").date().and_hms_opt(15, 30, 0).unwrap())
        );
        assert_eq!(parse_published_at(Some("")), None);
        assert_eq!(parse_published_at(Some("  ")), None);
        assert_eq!(parse_published_at(Some("not a date")), None);
        assert_eq!(parse_published_at(None), None);
    }

    #[test]
    fn explicit_date_always_wins() {
        let explicit = Some(dt("2026-01-15"));
        let existing = Some(dt("2025-12-01"));
        assert_eq!(resolve_published_at(explicit, existing, true), explicit);
        assert_eq!(resolve_published_at(explicit, existing, false), explicit);
    }

    #[test]
    fn publishing_without_date_keeps_existing_then_stamps_now() {
        let existing = Some(dt("2025-12-01"));
        assert_eq!(resolve_published_at(None, existing, true), existing);
        // No explicit or existing date → stamped with the current time.
        assert!(resolve_published_at(None, None, true).is_some());
    }

    #[test]
    fn unpublished_without_date_clears() {
        assert_eq!(resolve_published_at(None, Some(dt("2025-12-01")), false), None);
    }
}
