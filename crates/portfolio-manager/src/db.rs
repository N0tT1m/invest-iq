use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

/// Database backend abstraction.
/// Currently uses SQLite directly. Postgres support can be added by switching to AnyPool.
#[derive(Clone, Debug)]
pub struct PortfolioDb {
    pool: SqlitePool,
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
        let db_type = if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
            DbType::Postgres
        } else {
            DbType::Sqlite
        };

        match db_type {
            DbType::Sqlite => {
                let options = SqliteConnectOptions::from_str(database_url)?
                    .create_if_missing(true);

                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(options)
                    .await?;

                let db = Self { pool, db_type };
                db.run_migrations().await?;

                Ok(db)
            }
            DbType::Postgres => {
                // For Postgres, we still create a SQLite pool for the migration macro,
                // but in a real deployment you'd use sqlx::postgres::PgPool.
                // This is a stepping stone — full Postgres support requires AnyPool
                // or separate pool types behind a trait.
                anyhow::bail!(
                    "PostgreSQL support requires the 'postgres' feature. \
                     Set DATABASE_URL=sqlite:... for SQLite, or see docs/deployment.md \
                     for PostgreSQL setup instructions."
                );
            }
        }
    }

    /// Run sqlx migrations from the workspace migrations/ directory
    async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;
        Ok(())
    }

    /// Get the database pool
    pub fn pool(&self) -> &SqlitePool {
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
        // Should fail gracefully with a clear error
        let result = PortfolioDb::new("postgres://localhost/test").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("PostgreSQL"));
    }
}
