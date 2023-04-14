use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliOptions {
    #[clap(long, default_value = "80")]
    pub port: u16,

    #[clap(long, default_value = "postgresql://localhost:5432/graphix")]
    pub database_url: String,
}
