pub mod models;
pub mod schema;

use diesel::{
    connection::SimpleConnection,
    r2d2::{self, ConnectionManager, CustomizeConnection},
    sqlite::SqliteConnection,
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::error::{ApiResult, Error};

pub type Conn = SqliteConnection;
type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/sqlite");

#[derive(Debug)]
struct SqlitePragmas;

impl CustomizeConnection<SqliteConnection, r2d2::Error> for SqlitePragmas {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), r2d2::Error> {
        // busy_timeout must come first so the remaining pragmas wait instead of failing.
        conn.batch_execute(
            "PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
        .map_err(r2d2::Error::QueryError)
    }
}

/// Connection pool handle. Diesel is synchronous, so every query runs on the
/// blocking thread pool, the same pattern vaultwarden uses.
#[derive(Clone)]
pub struct Db {
    pool: Pool,
}

impl Db {
    pub fn connect(database_url: &str) -> Result<Self, String> {
        // WAL mode is persistent; switch once on a dedicated connection and migrate
        // before the pool opens its connections concurrently.
        {
            use diesel::Connection;
            let mut conn = SqliteConnection::establish(database_url)
                .map_err(|e| format!("cannot open database {database_url}: {e}"))?;
            conn.batch_execute("PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL;")
                .map_err(|e| format!("cannot enable WAL: {e}"))?;
            conn.run_pending_migrations(MIGRATIONS).map_err(|e| format!("migration failed: {e}"))?;
        }

        let manager = ConnectionManager::<SqliteConnection>::new(database_url);
        let pool = r2d2::Pool::builder()
            .max_size(10)
            .connection_customizer(Box::new(SqlitePragmas))
            .build(manager)
            .map_err(|e| format!("cannot open database {database_url}: {e}"))?;
        Ok(Self {
            pool,
        })
    }

    /// Run `f` with a pooled connection on the blocking pool.
    pub async fn run<T, F>(&self, f: F) -> ApiResult<T>
    where
        F: FnOnce(&mut Conn) -> ApiResult<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            f(&mut conn)
        })
        .await?
    }

    /// Run `f` inside `BEGIN IMMEDIATE`, so writers serialise up front instead
    /// of failing on a lock upgrade halfway through.
    pub async fn write<T, F>(&self, f: F) -> ApiResult<T>
    where
        F: FnOnce(&mut Conn) -> ApiResult<T> + Send + 'static,
        T: Send + 'static,
    {
        self.run(move |conn| conn.immediate_transaction::<T, Error, _>(f)).await
    }

    /// Run `f` in a read transaction so it sees one consistent snapshot.
    pub async fn read<T, F>(&self, f: F) -> ApiResult<T>
    where
        F: FnOnce(&mut Conn) -> ApiResult<T> + Send + 'static,
        T: Send + 'static,
    {
        use diesel::Connection;
        self.run(move |conn| conn.transaction::<T, Error, _>(f)).await
    }
}
