use chrono::{DateTime, Duration, Local, NaiveDateTime};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use std::sync::OnceLock;

use crate::config::Config;
use crate::models::Content;

pub type DbPool = Pool<SqliteConnectionManager>;

static DB_POOL: OnceLock<DbPool> = OnceLock::new();


pub fn init_pool_with_config(config: &Config) -> Result<DbPool, Box<dyn std::error::Error>> {
    let manager = SqliteConnectionManager::file(&config.db_path);
    let pool = Pool::builder()
        .max_size(10)
        .build(manager)?;
    
    // Initialize database schema
    let conn = pool.get()?;
    initialize_database(&conn)?;
    
    // Store the pool in the static variable
    DB_POOL.set(pool.clone())
        .map_err(|_| "Database pool already initialized")?;
    
    Ok(pool)
}

pub fn get_pool() -> &'static DbPool {
    DB_POOL.get().expect("Database pool not initialized")
}


pub fn initialize_database(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS articles (
            slug     TEXT PRIMARY KEY,
            title    TEXT NOT NULL,
            content  TEXT NOT NULL,
            createdAt DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS locks (
            title     TEXT NOT NULL,
            createdAt DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )?;

    Ok(())
}

pub fn get_article_by_slug(pool: &DbPool, slug: &str) -> Result<Content, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT title, content FROM articles WHERE slug = ?1 LIMIT 1")?;
    let result = stmt.query_row(params![slug], |row| {
        Ok(Content {
            title: row.get(0)?,
            content: row.get(1)?,
        })
    })?;
    Ok(result)
}

pub fn get_recent_articles(pool: &DbPool) -> Result<Vec<(String, String)>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT title, slug FROM articles ORDER BY createdAt DESC LIMIT 20")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    
    let mut articles = Vec::new();
    for row in rows {
        articles.push(row?);
    }
    Ok(articles)
}

pub fn check_daily_rate_limit(pool: &DbPool) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    let result = conn
        .prepare("SELECT createdAt FROM articles WHERE createdAt > datetime('now','-1 day') LIMIT 1")?
        .query_row([], |row| row.get::<usize, String>(0));
    
    match result {
        Ok(date_str) => Ok(Some(date_str)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn check_generation_lock(pool: &DbPool) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    let result = conn
        .prepare("SELECT createdAt FROM locks WHERE createdAt > datetime('now','-5 minutes') LIMIT 1")?
        .query_row([], |row| row.get::<usize, String>(0));
    
    match result {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn create_generation_lock(pool: &DbPool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    conn.execute("INSERT INTO locks (title) VALUES (?1)", params!["lock"])?;
    Ok(())
}

pub fn insert_article(pool: &DbPool, slug: &str, title: &str, content: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT INTO articles (slug, title, content) VALUES (?1, ?2, ?3)",
        params![slug, title, content],
    )?;
    Ok(())
}

pub fn calculate_wait_time(last_article_date: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let date = NaiveDateTime::parse_from_str(last_article_date, "%Y-%m-%d %H:%M:%S")?;
    let current_time = Local::now();
    let offset = current_time.offset().clone();
    let datetime = DateTime::<Local>::from_naive_utc_and_offset(date, offset) + Duration::days(1);
    
    let difference = datetime.signed_duration_since(current_time);
    Ok(difference.num_hours() + 1)
}