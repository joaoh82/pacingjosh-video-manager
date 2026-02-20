use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use log::info;
use std::path::Path;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
#[allow(dead_code)]
pub type DbConn = PooledConnection<ConnectionManager<SqliteConnection>>;

pub fn create_pool(database_url: &str) -> DbPool {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(database_url).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    r2d2::Pool::builder()
        .max_size(10)
        .build(manager)
        .expect("Failed to create database pool")
}

pub fn init_db(pool: &DbPool) {
    let mut conn = pool.get().expect("Failed to get DB connection");

    // Enable WAL mode and foreign keys for SQLite
    diesel::sql_query("PRAGMA journal_mode=WAL;")
        .execute(&mut conn)
        .ok();
    diesel::sql_query("PRAGMA foreign_keys=ON;")
        .execute(&mut conn)
        .ok();

    // Run embedded migrations
    run_migrations(&mut conn);

    info!("Database initialized");
}

fn run_migrations(conn: &mut SqliteConnection) {
    use diesel::connection::SimpleConnection;

    let up_sql = include_str!("../migrations/001_create_tables/up.sql");

    // Split by semicolons and execute each statement
    // Skip empty statements
    for statement in up_sql.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() {
            // Ignore errors from "already exists" on re-runs
            conn.batch_execute(&format!("{};", stmt)).ok();
        }
    }

    info!("Migrations applied");
}
