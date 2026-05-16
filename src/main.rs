mod shared;
pub mod client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    client::cli::run_cli().await?;
    Ok(())
}
