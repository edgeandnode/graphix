use std::ops::Deref;

use graphix_store::Store;
use testcontainers::clients::Cli;
use testcontainers::Container;

/// A wrapper around a [`Store`] that is backed by a containerized Postgres
/// database.
pub struct EmptyStoreForTesting<'a> {
    _container: Container<'a, testcontainers_modules::postgres::Postgres>,
    store: Store,
}

impl<'a> EmptyStoreForTesting<'a> {
    pub async fn new(docker_client: &'a Cli) -> anyhow::Result<EmptyStoreForTesting<'a>> {
        use testcontainers_modules::postgres::Postgres;

        let container = docker_client.run(Postgres::default());
        let connection_string = &format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            container.get_host_port_ipv4(5432)
        );
        let store = Store::new(connection_string).await?;
        Ok(Self {
            _container: container,
            store,
        })
    }
}

impl<'a> Deref for EmptyStoreForTesting<'a> {
    type Target = Store;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}
