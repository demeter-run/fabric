use anyhow::Result;
use dotenv::dotenv;
use tracing::{info, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let env_filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .with_env_var("RUST_LOG")
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(env_filter)
        .init();

    let grpc_driver = tokio::spawn(async { fabric::drivers::grpc::server().await });

    let event_driver = tokio::spawn(async { fabric::drivers::event::subscribe().await });

    info!("rpc services running");
    let _result = futures::future::join(grpc_driver, event_driver).await;

    Ok(())
}
