use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    fabric::drivers::grpc::server().await?;

    Ok(())
}
