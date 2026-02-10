use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

#[derive(Clone)]
pub struct PortfolioDb {
    pool: SqlitePool,
}

impl PortfolioDb {
    /// Create a new database connection
    pub async fn new(database_url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.init_schema().await?;

        Ok(db)
    }

    /// Initialize database schema
    async fn init_schema(&self) -> Result<()> {
        let schema = include_str!("../../../schema.sql");

        // Execute schema (split by statement since sqlx doesn't support multiple statements)
        for statement in schema.split(';') {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                sqlx::query(stmt).execute(&self.pool).await?;
            }
        }

        Ok(())
    }

    /// Get the database pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Check if database file exists
    pub fn exists(path: &str) -> bool {
        // Remove "sqlite:" prefix if present
        let file_path = path.strip_prefix("sqlite:").unwrap_or(path);
        Path::new(file_path).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_creation() {
        let db = PortfolioDb::new("sqlite::memory:").await.unwrap();
        assert!(db.pool().acquire().await.is_ok());
    }
}
