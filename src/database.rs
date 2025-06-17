use chrono::{DateTime, Duration, Local, NaiveDateTime};
use rusqlite::{params, Connection};
use std::env;

use crate::models::Content;

pub fn create_connection() -> Result<Connection, rusqlite::Error> {
    let db_path = env::var("DB_PATH").unwrap_or("./blog.db".to_string());
    Connection::open(db_path)
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

pub fn get_article_by_slug(conn: &Connection, slug: &str) -> Result<Content, rusqlite::Error> {
    conn.prepare("SELECT title, content FROM articles WHERE slug = ?1 LIMIT 1")?
        .query_row(params![slug], |row| {
            Ok(Content {
                title: row.get(0)?,
                content: row.get(1)?,
            })
        })
}

pub fn get_recent_articles(conn: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
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

pub fn check_daily_rate_limit(conn: &Connection) -> Result<Option<String>, rusqlite::Error> {
    let result = conn
        .prepare("SELECT createdAt FROM articles WHERE createdAt > datetime('now','-1 day') LIMIT 1")?
        .query_row([], |row| row.get::<usize, String>(0));
    
    match result {
        Ok(date_str) => Ok(Some(date_str)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn check_generation_lock(conn: &Connection) -> Result<bool, rusqlite::Error> {
    let result = conn
        .prepare("SELECT createdAt FROM locks WHERE createdAt > datetime('now','-5 minutes') LIMIT 1")?
        .query_row([], |row| row.get::<usize, String>(0));
    
    match result {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn create_generation_lock(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute("INSERT INTO locks (title) VALUES (?1)", params!["lock"])?;
    Ok(())
}

pub fn insert_article(conn: &Connection, slug: &str, title: &str, content: &str) -> Result<(), rusqlite::Error> {
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