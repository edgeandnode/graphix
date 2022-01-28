use anyhow;
use warp::Filter;

pub async fn run() -> Result<(), anyhow::Error> {
    let hello = warp::path!("hello" / String).map(|name| format!("Hello, {}!", name));

    Ok(warp::serve(hello).run(([127, 0, 0, 1], 3031)).await)
}
