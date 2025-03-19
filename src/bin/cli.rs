use std::{collections::HashMap, path::PathBuf};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use fabric::drivers::{
    backoffice::{BackofficeConfig, BackofficeTlsConfig, OutputFormat},
    cache::CacheConfig,
};
use serde::Deserialize;
use tracing::Level;
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
pub struct UsageArgs {
    /// period to collect the data (year-month) e.g 2024-09
    pub period: String,

    /// format that will be returned table(log in terminal), json(log in terminal), csv(save a file e.g 2024-09.csv)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Parser, Clone)]
pub struct ProjectArgs {
    /// Project namespace
    #[arg(short, long)]
    pub namespace: Option<String>,

    /// By any resource spec value
    #[arg(short, long)]
    pub spec: Option<String>,

    /// Resource name
    #[arg(short, long)]
    pub resource_name: Option<String>,

    /// User email
    #[arg(short, long)]
    pub email: Option<String>,
}

#[derive(Parser, Clone)]
pub struct ResourceArgs {
    /// Project namespace
    #[arg(short, long)]
    pub namespace: Option<String>,

    /// By any resource spec value
    #[arg(short, long)]
    pub spec: Option<String>,
}

#[derive(Parser, Clone)]
pub struct NewUsersArgs {
    /// collect new users after this date (year-month-day) e.g 2024-09-01
    pub after: String,

    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Parser, Clone)]
pub struct DiffArgs {
    /// csv or table
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync cache
    Sync,

    /// Get the usage data
    Usage(UsageArgs),

    /// Get projects by user
    Project(ProjectArgs),

    /// Get resource by project namespace
    Resource(ResourceArgs),

    /// Get new users since a date
    NewUsers(NewUsersArgs),

    /// Check the diff of the state with the cluster
    Diff(DiffArgs),
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
        Commands::Diff(args) => {
            let output = match args.output {
                Some(output) => match output.as_str() {
                    "table" => OutputFormat::Table,
                    "csv" => OutputFormat::Csv,
                    _ => bail!("invalid output format"),
                },
                None => OutputFormat::Table,
            };

            fabric::drivers::backoffice::fetch_diff(config.clone().into(), output).await?;
        }
        Commands::Usage(args) => {
            let output = match args.output {
                Some(output) => match output.as_str() {
                    "table" => OutputFormat::Table,
                    "json" => OutputFormat::Json,
                    "csv" => OutputFormat::Csv,
                    _ => bail!("invalid output format"),
                },
                None => OutputFormat::Table,
            };

            fabric::drivers::backoffice::fetch_usage(config.clone().into(), &args.period, output)
                .await?;
        }
        Commands::Project(args) => {
            fabric::drivers::backoffice::fetch_projects(
                config.clone().into(),
                args.namespace,
                args.spec,
                args.resource_name,
                args.email,
            )
            .await?;
        }
        Commands::Resource(args) => {
            fabric::drivers::backoffice::fetch_resources(
                config.clone().into(),
                args.namespace,
                args.spec,
            )
            .await?;
        }
        Commands::NewUsers(args) => {
            let output = match args.output {
                Some(output) => match output.as_str() {
                    "table" => OutputFormat::Table,
                    "json" => OutputFormat::Json,
                    "csv" => OutputFormat::Csv,
                    _ => bail!("invalid output format"),
                },
                None => OutputFormat::Table,
            };

            fabric::drivers::backoffice::fetch_new_users(
                config.clone().into(),
                &args.after,
                output,
            )
            .await?;
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

impl From<Config> for BackofficeConfig {
    fn from(value: Config) -> Self {
        Self {
            db_path: value.db_path,
            kafka: value.kafka_consumer,
            topic: value.topic,
            tls_config: value.tls.map(|value| BackofficeTlsConfig {
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
