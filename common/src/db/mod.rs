use diesel::{
    r2d2::{self, ConnectionManager, Pool, PooledConnection},
    PgConnection,
};
use std::sync::Arc;
use tracing::info;

pub mod models;
pub mod proofs_of_indexing;
pub mod schema;

embed_migrations!("../migrations");

/// An abstraction over all database operations. It uses [`Arc`] internally, so
/// it's cheaply cloneable.
pub struct Store {
    pool: Arc<Pool<ConnectionManager<PgConnection>>>,
}

impl Store {
    pub fn new(db_url: &str) -> anyhow::Result<Self> {
        let manager = r2d2::ConnectionManager::<PgConnection>::new(db_url);
        let pool = Arc::new(r2d2::Builder::new().build(manager)?);
        let store = Self { pool };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> anyhow::Result<()> {
        info!("Run database migrations");
        let connection = self.pool.get()?;
        embedded_migrations::run(&connection)?;
        Ok(())
    }

    pub fn conn(&self) -> anyhow::Result<PooledConnection<ConnectionManager<PgConnection>>> {
        Ok(self.pool.get()?)
    }
}
