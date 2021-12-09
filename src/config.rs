use reqwest::Url;
use serde::{Deserialize, Deserializer};
use std::{fs::File, path::PathBuf};

fn deserialize_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Url::parse(&s).map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct IndexerUrls {
    #[serde(deserialize_with = "deserialize_url")]
    pub status: Url,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnvironmentConfig {
    pub id: String,
    pub urls: IndexerUrls,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestingConfig {
    pub database_url: String,
    pub environments: Vec<EnvironmentConfig>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfig {}

#[derive(Debug, Deserialize)]
pub struct P2PIndexerConfig {}

#[derive(Debug, Deserialize)]
pub struct P2PConfig {
    #[serde(default)]
    pub indexers: Vec<P2PIndexerConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Config {
    Testing(TestingConfig),
    Network(NetworkConfig),
    P2P(P2PConfig),
}

impl TryFrom<&PathBuf> for Config {
    type Error = anyhow::Error;

    fn try_from(path: &PathBuf) -> Result<Config, Self::Error> {
        let file = File::open(path)?;
        Ok(serde_yaml::from_reader(file)
            .map_err(|e| Self::Error::new(e).context("invalid config file"))?)
    }
}
