use std::{collections::HashMap, path::PathBuf};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use fabric::drivers::{
    billing::{BillingConfig, BillingTlsConfig, OutputFormat},
    cache::CacheConfig,
};
use serde::Deserialize;
use tracing::{info, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[arg(short, long, help = "Cli config path file", env = "CLI_CONFIG")]
    config: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Clone)]
pub struct BillingArgs {
    /// period to collect the data (month-year) e.g 09-2024
    pub period: String,

    /// format that will be returned table(log in terminal), json(log in terminal), csv(save a file e.g 09-2024.csv)
    pub output: String,
}
#[derive(Subcommand)]
enum Commands {
    /// Send the billing invoices
    Billing(BillingArgs),
}

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

    let cli = Cli::parse();
    let config = Config::new(&cli.config)?;

    match cli.command {
        Commands::Billing(args) => {
            info!("sincronizing cache");

            let output = match args.output.as_str() {
                "table" => OutputFormat::Table,
                "json" => OutputFormat::Json,
                "csv" => OutputFormat::Csv,
                _ => bail!("invalid output format"),
            };

            fabric::drivers::cache::subscribe(config.clone().into()).await?;
            fabric::drivers::billing::run(config.clone().into(), &args.period, output).await?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct TlsConfig {
    ssl_crt_path: PathBuf,
    ssl_key_path: PathBuf,
}
#[derive(Debug, Clone, Deserialize)]
struct Config {
    db_path: String,
    topic: String,
    tls: Option<TlsConfig>,
    kafka_consumer: HashMap<String, String>,
}
impl Config {
    pub fn new(path: &str) -> Result<Self> {
        let config = config::Config::builder()
            .add_source(config::File::with_name(path).required(true))
            .build()?
            .try_deserialize()?;

        Ok(config)
    }
}

impl From<Config> for BillingConfig {
    fn from(value: Config) -> Self {
        Self {
            db_path: value.db_path,
            kafka: value.kafka_consumer,
            topic: value.topic,
            tls_config: value.tls.map(|value| BillingTlsConfig {
                ssl_key_path: value.ssl_key_path,
                ssl_crt_path: value.ssl_crt_path,
            }),
        }
    }
}

impl From<Config> for CacheConfig {
    fn from(value: Config) -> Self {
        Self {
            kafka: value.kafka_consumer,
            db_path: value.db_path,
            topic: value.topic,
        }
    }
}
