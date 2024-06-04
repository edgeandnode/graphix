use graphix_store::Store;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

const POSTGRES_PORT: u16 = 5432;

/// A wrapper around a [`Store`] that is backed by a containerized Postgres
/// database.
#[derive(derive_more::Deref)]
pub struct EmptyStoreForTesting {
    #[deref]
    store: Store,
    _container: ContainerAsync<Postgres>,
}

impl EmptyStoreForTesting {
    pub async fn new() -> anyhow::Result<Self> {
        let container = Postgres::default().start().await?;
        let connection_string = &format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            container.get_host_port_ipv4(POSTGRES_PORT).await?
        );

        let store = Store::new(connection_string).await?;
        Ok(Self {
            _container: container,
            store,
        })
    }
}
