use eventuals::*;
use std::time::Duration;
use tracing::*;

use crate::config::TestingConfig;
use crate::indexer::RealIndexer;

#[instrument]
pub fn testing_indexers(config: TestingConfig) -> Eventual<Vec<RealIndexer>> {
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
                    .map(RealIndexer::new)
                    .collect::<Vec<_>>(),
            );
        }
    });

    eventual
}
