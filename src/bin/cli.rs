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
    /// period to collect the data (year-month) e.g 2024-09
    pub period: String,

    /// format that will be returned table(log in terminal), json(log in terminal), csv(save a file e.g 2024-09.csv)
    pub output: String,
}

#[derive(Parser, Clone)]
pub struct ProjectArgs {
    /// Owner user email
    pub email: String,
}

#[derive(Parser, Clone)]
pub struct ResourceArgs {
    /// Project namespace
    pub namespace: String,
}

#[derive(Parser, Clone)]
pub struct NewUsersArgs {
    /// collect new users after this date (year-month-day) e.g 2024-09-01
    pub after: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync cache
    Sync,

    /// Get the billing data
    Billing(BillingArgs),

    /// Get projects by user
    Project(ProjectArgs),

    /// Get resource by project namespace
    Resource(ResourceArgs),

    /// Get new users since a date
    NewUsers(NewUsersArgs),
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
        Commands::Sync => {
            fabric::drivers::cache::subscribe(config.clone().into()).await?;
        }
        Commands::Billing(args) => {
            info!("sincronizing cache");

            let output = match args.output.as_str() {
                "table" => OutputFormat::Table,
                "json" => OutputFormat::Json,
                "csv" => OutputFormat::Csv,
                _ => bail!("invalid output format"),
            };

            fabric::drivers::billing::fetch_usage(config.clone().into(), &args.period, output)
                .await?;
        }
        Commands::Project(args) => {
            fabric::drivers::billing::fetch_projects(config.clone().into(), &args.email).await?;
        }
        Commands::Resource(args) => {
            fabric::drivers::billing::fetch_resources(config.clone().into(), &args.namespace)
                .await?;
        }
        Commands::NewUsers(args) => {
            fabric::drivers::billing::fetch_new_users(config.clone().into(), &args.after).await?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct AuthConfig {
    url: String,
    client_id: String,
    client_secret: String,
    audience: String,
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
    auth: AuthConfig,
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
            auth_url: value.auth.url,
            auth_client_id: value.auth.client_id,
            auth_client_secret: value.auth.client_secret,
            auth_audience: value.auth.audience,
        }
    }
}

impl From<Config> for CacheConfig {
    fn from(value: Config) -> Self {
        Self {
            kafka: value.kafka_consumer,
            db_path: value.db_path,
            topic: value.topic,
            notify: None,
        }
    }
}
