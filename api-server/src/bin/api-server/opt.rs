use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Options {
    #[structopt(long, default_value = "80")]
    pub port: u16,

    #[structopt(long, default_value = "postgresql://localhost:5432/graph-ixi")]
    pub database_url: String,
}

pub fn options_from_args() -> Options {
    Options::from_args()
}
