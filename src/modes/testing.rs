use eventuals::*;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tracing::*;

use crate::config::{EnvironmentConfig, TestingConfig};
use crate::indexer::Indexer;

#[instrument]
pub fn testing_indexers(config: TestingConfig) -> Eventual<Vec<Arc<Indexer>>> {
    let (mut out, eventual) = Eventual::new();

    tokio::spawn(async move {
        // Sync indexing statuses from test environments periodically
        let mut timer = timer(Duration::from_secs(120)).subscribe();
        loop {
            timer.next().await.unwrap();

            info!("Refresh indexers");

            out.write(
                config
                    .environments
                    .iter()
                    .map(Indexer::from_environment)
                    .collect::<FuturesUnordered<_>>()
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .zip(config.environments.iter())
                    .filter_map(skip_errors)
                    .map(Arc::new)
                    .collect(),
            );
        }
    });

    eventual
}

fn skip_errors(result: (Result<Indexer, anyhow::Error>, &EnvironmentConfig)) -> Option<Indexer> {
    match result.0 {
        Ok(indexer) => {
            let Indexer { id, urls, .. } = &indexer;
            let url = urls.status.to_string();
            info!(%id, %url, "Successfully refreshed indexer");

            Some(indexer)
        }
        Err(error) => {
            let EnvironmentConfig { id, urls } = result.1;
            let url = urls.status.to_string();
            warn!(%id, %url, %error, "Failed to refresh indexer");
            None
        }
    }
}
