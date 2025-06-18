use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use dotenv::dotenv;
use log::{info, warn, error, debug};
use std::env;
use std::time::Duration;

#[derive(Debug)]
pub struct ConnectionOptions {
    pub enable_wal: bool,
    pub enable_foreign_keys: bool,
    pub busy_timeout: Option<Duration>,
}

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for ConnectionOptions
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        debug!("🔧 Customizing new database connection");
        
        (|| {
            if self.enable_wal {
                debug!("🔧 Enabling WAL mode and setting synchronous to NORMAL");
                conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
            }
            if self.enable_foreign_keys {
                debug!("🔧 Enabling foreign key constraints");
                conn.batch_execute("PRAGMA foreign_keys = ON;")?;
            }
            if let Some(d) = self.busy_timeout {
                debug!("🔧 Setting busy timeout to {}ms", d.as_millis());
                conn.batch_execute(&format!("PRAGMA busy_timeout = {};", d.as_millis()))?;
            }
            debug!("✅ Database connection customization completed");
            Ok(())
        })()
        .map_err(|e| {
            error!("❌ Failed to customize database connection: {}", e);
            diesel::r2d2::Error::QueryError(e)
        })
    }
}

// Type alias for the pool type
pub type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

// Function to establish a connection pool
pub fn establish_connection_pool() -> DbPool {
    info!("🔗 Establishing database connection pool");
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        warn!("⚠️ DATABASE_URL not set, using default: data/stats.sqlite");
        "data/stats.sqlite".to_string()
    });
    
    info!("🔗 Database URL: {}", database_url);
    
    let manager = ConnectionManager::<SqliteConnection>::new(&database_url);
    debug!("🔧 Created connection manager for: {}", database_url);

    let pool_config = r2d2::Pool::builder()
        .max_size(16)
        .connection_customizer(Box::new(ConnectionOptions {
            enable_wal: true,
            enable_foreign_keys: true,
            busy_timeout: Some(Duration::from_secs(30)),
        }));
    
    info!("🔧 Pool configuration - Max size: 16, WAL enabled, Foreign keys enabled, Timeout: 30s");

    match pool_config.build(manager) {
        Ok(pool) => {
            info!("✅ Database connection pool created successfully");
            // Test the connection
            match pool.get() {
                Ok(_conn) => {
                    info!("✅ Database connection test successful");
                }
                Err(e) => {
                    error!("❌ Database connection test failed: {}", e);
                }
            }
            pool
        }
        Err(e) => {
            error!("❌ Failed to create database connection pool: {}", e);
            panic!("Database connection pool creation failed: {}", e);
        }
    }
}
