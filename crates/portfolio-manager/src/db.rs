use anyhow::Result;
use std::path::Path;

/// Type alias for the database pool used across the application.
pub type DbPool = sqlx::AnyPool;

/// Database backend abstraction.
/// Uses sqlx AnyPool for runtime backend selection (SQLite or PostgreSQL).
#[derive(Clone, Debug)]
pub struct PortfolioDb {
    pool: sqlx::AnyPool,
    db_type: DbType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DbType {
    Sqlite,
    Postgres,
}

impl PortfolioDb {
    /// Create a new database connection.
    /// Supports both SQLite (`sqlite:...`) and PostgreSQL (`postgres://...`) URLs.
    pub async fn new(database_url: &str) -> Result<Self> {
        sqlx::any::install_default_drivers();

        let db_type = if database_url.starts_with("postgres://")
            || database_url.starts_with("postgresql://")
        {
            DbType::Postgres
        } else {
            DbType::Sqlite
        };

        let (max_conns, url) = match db_type {
            DbType::Postgres => (20, database_url.to_string()),
            DbType::Sqlite => {
                // Ensure create-if-missing for SQLite via mode=rwc
                let u = if database_url.contains(":memory:") || database_url.contains("mode=") {
                    database_url.to_string()
                } else if database_url.contains('?') {
                    format!("{}&mode=rwc", database_url)
                } else {
                    format!("{}?mode=rwc", database_url)
                };
                (5, u)
            }
        };

        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(max_conns)
            .connect(&url)
            .await?;

        let db = Self { pool, db_type };
        db.run_migrations().await?;

        Ok(db)
    }

    /// Run sqlx migrations from the workspace migrations/ directory
    async fn run_migrations(&self) -> Result<()> {
        match self.db_type {
            DbType::Sqlite => {
                sqlx::migrate!("../../migrations/sqlite")
                    .run(&self.pool)
                    .await
                    .map_err(|e| anyhow::anyhow!("SQLite migration failed: {}", e))?;
            }
            DbType::Postgres => {
                sqlx::migrate!("../../migrations/postgres")
                    .run(&self.pool)
                    .await
                    .map_err(|e| anyhow::anyhow!("PostgreSQL migration failed: {}", e))?;
            }
        }
        Ok(())
    }

    /// Get the database pool
    pub fn pool(&self) -> &sqlx::AnyPool {
        &self.pool
    }

    /// Get the database type
    pub fn db_type(&self) -> DbType {
        self.db_type
    }

    /// Check if database file exists (SQLite only)
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
        assert_eq!(db.db_type(), DbType::Sqlite);
    }

    #[tokio::test]
    async fn test_postgres_url_detection() {
        // PostgreSQL URL should be detected but will fail to connect (no server)
        let result = PortfolioDb::new("postgres://localhost:59999/nonexistent").await;
        assert!(result.is_err());
    }
}
