use std::{collections::HashMap, path::PathBuf};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use fabric::drivers::{
    backoffice::{BackofficeConfig, OutputFormat},
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
pub struct RenameProjectArgs {
    /// Project id
    #[arg(short, long)]
    pub id: String,

    /// New name
    #[arg(short, long)]
    pub new_name: String,

    // Dry run
    #[arg(short, long, action)]
    pub dry_run: bool,
}

#[derive(Parser, Clone)]
pub struct ProjectUsersArgs {
    /// Project id
    #[arg(short, long)]
    pub id: String,

    /// table(log in terminal), csv(save a file project-users.csv)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Parser, Clone)]
pub struct InviteUserArgs {
    /// Project id
    #[arg(short, long)]
    pub id: String,

    /// Email to invite. The invitee must sign in with this exact address to accept.
    #[arg(short, long)]
    pub email: String,

    /// Role to grant on acceptance: owner or member
    #[arg(short, long, default_value = "member")]
    pub role: String,

    /// Minutes the invite code stays valid. Defaults to [email].invite_ttl_min, then to 15.
    #[arg(short, long)]
    pub ttl_min: Option<u64>,

    // Dry run
    #[arg(short, long, action)]
    pub dry_run: bool,
}

#[derive(Parser, Clone)]
pub struct TransferProjectArgs {
    /// Project id
    #[arg(short, long)]
    pub id: String,

    /// Email of the new owner. Must already be a member of the project.
    #[arg(short, long)]
    pub new_owner_email: String,

    /// Leave the Stripe customer untouched. Use when the billing contact was set deliberately
    /// rather than tracking the project owner, since the update overwrites name and email.
    #[arg(short, long, action)]
    pub skip_stripe: bool,

    // Dry run
    #[arg(short, long, action)]
    pub dry_run: bool,
}

#[derive(Parser, Clone)]
pub struct DeleteProjectArgs {
    /// Project id
    #[arg(short, long)]
    pub id: String,

    // Dry run
    #[arg(short, long, action)]
    pub dry_run: bool,
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
pub struct DeleteResourceArgs {
    /// UUID of the resource to delete.
    #[arg(short, long)]
    pub id: String,

    /// ID of the project to delete.
    #[arg(short, long)]
    pub project_id: String,

    // Dry run
    #[arg(short, long, action)]
    pub dry_run: bool,
}

#[derive(Parser, Clone)]
pub struct PatchResourceArgs {
    /// UUID of the resource to patch.
    #[arg(short, long)]
    pub id: String,

    /// ID of the project of the resource to patch.
    #[arg(short, long)]
    pub project_id: String,

    /// JSON patch of the resource spec.
    #[arg(short, long)]
    pub patch: String,

    // Dry run
    #[arg(short, long, action)]
    pub dry_run: bool,
}

#[derive(Parser, Clone)]
pub struct CreateResourceArgs {
    /// ID of the project to create the resource in.
    #[arg(short, long)]
    pub project_id: String,

    /// Kind of the resource to create.
    /// E.g: "BlockfrostPort", "CardanoNodePort", "OgmiosPort", etc
    #[arg(short, long)]
    pub kind: String,

    /// Spec of the resource to create.
    /// This should be a JSON string.
    /// E.g: '{"network":"cardano-preview","throughputTier":"0","operatorVersion":"1"}'
    #[arg(short, long)]
    pub spec: String,

    // Dry run
    #[arg(short, long, action)]
    pub dry_run: bool,
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

    /// List the users of a project with their roles
    ProjectUsers(ProjectUsersArgs),

    /// Get projects by user
    RenameProject(RenameProjectArgs),

    /// Invite a user to a project by email
    InviteUser(InviteUserArgs),

    /// Transfer a project to another member of the project
    TransferProject(TransferProjectArgs),

    /// Get resource by project namespace
    Resource(ResourceArgs),

    /// Send patch for resource
    PatchResource(PatchResourceArgs),

    /// Create a new resource
    CreateResource(CreateResourceArgs),

    /// Get new users since a date
    NewUsers(NewUsersArgs),

    /// Check the diff of the state with the cluster
    Diff(DiffArgs),

    /// Delete project
    DeleteProject(DeleteProjectArgs),

    /// Delete resource
    DeleteResource(DeleteResourceArgs),
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
        Commands::PatchResource(args) => {
            fabric::drivers::backoffice::patch_resource(
                config.clone().into(),
                args.id,
                args.project_id,
                args.patch,
                args.dry_run,
            )
            .await?;
        }
        Commands::CreateResource(args) => {
            fabric::drivers::backoffice::create_resource(
                config.clone().into(),
                args.project_id,
                args.kind,
                args.spec,
                args.dry_run,
            ).await?
        }
        Commands::RenameProject(args) => {
            fabric::drivers::backoffice::rename_project(
                config.clone().into(),
                args.id,
                args.new_name,
                args.dry_run,
            )
            .await?;
        }
        Commands::ProjectUsers(args) => {
            let output = match args.output {
                Some(output) => match output.as_str() {
                    "table" => OutputFormat::Table,
                    "csv" => OutputFormat::Csv,
                    _ => bail!("invalid output format"),
                },
                None => OutputFormat::Table,
            };

            fabric::drivers::backoffice::fetch_project_users(
                config.clone().into(),
                args.id,
                output,
            )
            .await?;
        }
        Commands::InviteUser(args) => {
            fabric::drivers::backoffice::invite_user(
                config.clone().into(),
                args.id,
                args.email,
                args.role,
                args.ttl_min,
                args.dry_run,
            )
            .await?;
        }
        Commands::TransferProject(args) => {
            fabric::drivers::backoffice::transfer_project(
                config.clone().into(),
                args.id,
                args.new_owner_email,
                args.dry_run,
                args.skip_stripe,
            )
            .await?;
        }
        Commands::DeleteProject(args) => {
            fabric::drivers::backoffice::delete_project(
                config.clone().into(),
                args.id,
                args.dry_run,
            )
            .await?;
        }
        Commands::DeleteResource(args) => {
            fabric::drivers::backoffice::delete_resource(
                config.clone().into(),
                args.id,
                args.project_id,
                args.dry_run,
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
struct StripeConfig {
    url: String,
    api_key: String,
}
#[derive(Debug, Clone, Deserialize)]
struct EmailConfig {
    ses_access_key_id: String,
    ses_secret_access_key: String,
    ses_region: String,
    ses_verified_email: String,
    /// Default lifetime for `invite-user`, in minutes. Optional; `--ttl-min` wins when given.
    invite_ttl_min: Option<u64>,
}
#[derive(Debug, Clone, Deserialize)]
struct Config {
    db_path: String,
    topic_events: String,
    topic_usage: Option<String>,
    kafka_consumer: HashMap<String, String>,
    kafka_producer: HashMap<String, String>,
    auth: AuthConfig,
    /// Only required by `transfer-project`; every other subcommand works without it.
    stripe: Option<StripeConfig>,
    /// Only required by `invite-user`; every other subcommand works without it.
    email: Option<EmailConfig>,
    crds_path: PathBuf,
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
            crds_path: value.crds_path,
            auth_url: value.auth.url,
            auth_client_id: value.auth.client_id,
            auth_client_secret: value.auth.client_secret,
            auth_audience: value.auth.audience,
            stripe_url: value.stripe.as_ref().map(|s| s.url.clone()),
            stripe_api_key: value.stripe.as_ref().map(|s| s.api_key.clone()),
            ses_access_key_id: value.email.as_ref().map(|e| e.ses_access_key_id.clone()),
            ses_secret_access_key: value
                .email
                .as_ref()
                .map(|e| e.ses_secret_access_key.clone()),
            ses_region: value.email.as_ref().map(|e| e.ses_region.clone()),
            ses_verified_email: value.email.as_ref().map(|e| e.ses_verified_email.clone()),
            invite_ttl_min: value.email.as_ref().and_then(|e| e.invite_ttl_min),
            topic_events: value.topic_events,
            kafka_producer: value.kafka_producer,
        }
    }
}

impl From<Config> for CacheConfig {
    fn from(value: Config) -> Self {
        Self {
            kafka: value.kafka_consumer,
            db_path: value.db_path,
            topics: match value.topic_usage {
                Some(topic) => [value.topic_events, topic].to_vec(),
                None => [value.topic_events].to_vec(),
            },
            notify: None,
        }
    }
}
