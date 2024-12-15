use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, about, version)]
pub struct CliOptions {
    /// The URL of the PostgreSQL database to use. Can also be set via env.
    /// var..
    #[clap(long, env = "GRAPHIX_DB_URL")]
    pub database_url: String,
    /// The port on which the GraphQL API server should listen.
    #[clap(long, default_value_t = 8000)]
    pub port: u16,
    /// The port on which the Prometheus exporter should listen.
    #[clap(long, default_value_t = 9184)]
    pub prometheus_port: u16,
}
