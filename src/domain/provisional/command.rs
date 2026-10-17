use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use tracing::info;
use uuid::Uuid;

use crate::domain::{
    error::Error,
    event::{EventDrivenBridge, ProjectExpired, ProvisionalProjectCreated},
    project::{self, cache::ProjectDrivenCache, ProjectStatus},
    utils, Result,
};

use super::{
    display_user_code, gateway_token_matches, generate_claim_token, generate_user_code,
    hash_claim_token, hash_source_ip, ProvisionalConfig, GLOBAL_CAP, IP_CAP, REGISTRATION_DISABLED,
};

const AGENT_NAME_MAX: usize = 64;
const AGENT_FIELD_MAX: usize = 256;
const USER_CODE_ATTEMPTS: usize = 5;
/// The trailing window the per-IP cap counts registrations in.
const IP_CAP_WINDOW: Duration = Duration::hours(24);

/// Registers a provisional project for an anonymous agent and mints its pre-claim API key.
///
/// No Stripe customer and no Auth0 lookup: both happen at the claim, so neither is passed in.
/// Both caps read the projection, so a burst inside the event lag may slip one or two past them.
pub async fn register(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    config: &ProvisionalConfig,
    secret: &str,
    cmd: RegisterCmd,
) -> Result<Registration> {
    if !gateway_token_matches(&config.gateway_token, cmd.gateway_token.as_deref()) {
        return Err(Error::Unauthenticated("invalid gateway token".into()));
    }

    if !config.enabled {
        return Err(Error::Unavailable(REGISTRATION_DISABLED.into()));
    }

    cmd.validate()?;

    let now = Utc::now();

    let source_ip_hash = hash_source_ip(&cmd.client_ip, secret);
    let since = now - IP_CAP_WINDOW;
    let recent = cache
        .count_registrations_by_source(&source_ip_hash, &since)
        .await?;
    if recent >= config.ip_cap {
        // The window slides, so the source may register again once enough of its registrations
        // age out to bring it back under the cap. Usually that is the oldest one; past a burst
        // that slipped over the cap, it is the one `recent - ip_cap` places later.
        let freeing = cache
            .find_registration_created_at_by_source(&source_ip_hash, &since, recent - config.ip_cap)
            .await?;
        return Err(Error::RetryAfter {
            message: IP_CAP.into(),
            retry_after_seconds: ip_cap_retry_after(freeing, now),
        });
    }

    if cache.count_live_provisional().await? >= config.global_cap {
        return Err(Error::Unavailable(GLOBAL_CAP.into()));
    }

    let id = Uuid::new_v4().to_string();
    let namespace = utils::get_random_name();
    if cache.find_by_namespace(&namespace).await?.is_some() {
        return Err(Error::Unexpected(
            "invalid random namespace, try again".into(),
        ));
    }

    let claim_token = generate_claim_token();
    let user_code = unused_user_code(cache.clone()).await?;
    let expires_at = now + Duration::days(config.ttl_days.into());

    let evt = ProvisionalProjectCreated {
        id: id.clone(),
        name: namespace.clone(),
        namespace: namespace.clone(),
        status: ProjectStatus::Provisional.to_string(),
        agent_name: cmd.agent_name.clone(),
        agent_version: cmd.agent_version,
        agent_url: cmd.agent_url,
        expires_at,
        claim_token_hash: hash_claim_token(&claim_token),
        user_code: user_code.clone(),
        source_ip_hash,
        created_at: now,
        updated_at: now,
    };
    event.dispatch(evt.into()).await?;

    let api_key = project::command::create_pre_claim_secret(event, &id, secret.into()).await?;

    // The claim token, the user code and the key are deliberately not logged.
    info!(
        project = &namespace,
        agent = &cmd.agent_name,
        "new provisional project registered"
    );

    Ok(Registration {
        project_id: id,
        project_namespace: namespace,
        api_key,
        claim_token,
        user_code: display_user_code(&user_code),
        expires_at,
    })
}

/// Dispatches `ProjectExpired` for every provisional project whose expiry has passed, and returns
/// how many it expired.
///
/// Idempotent: an expired project is no longer provisional, so the next sweep skips it. A
/// duplicate, from a sweep that ran before the projection caught up, is a no-op downstream.
pub async fn sweep_expired(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    now: DateTime<Utc>,
) -> Result<usize> {
    let projects = cache.find_expired_provisional(&now).await?;

    for project in &projects {
        let evt = ProjectExpired {
            id: project.id.clone(),
            namespace: project.namespace.clone(),
            expired_at: now,
        };
        event.dispatch(evt.into()).await?;
        info!(project = &project.namespace, "provisional project expired");
    }

    Ok(projects.len())
}

/// Whole seconds until a registration created at `created_at` leaves the cap's window, rounded
/// up and at least one. Without one to wait for, as when the projection moved between the two
/// reads, the whole window.
fn ip_cap_retry_after(created_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> u64 {
    let Some(created_at) = created_at else {
        return IP_CAP_WINDOW.num_seconds().unsigned_abs();
    };
    let millis = (created_at + IP_CAP_WINDOW - now).num_milliseconds();
    let seconds = millis.div_euclid(1000) + i64::from(millis.rem_euclid(1000) > 0);
    seconds.max(1).unsigned_abs()
}

async fn unused_user_code(cache: Arc<dyn ProjectDrivenCache>) -> Result<String> {
    for _ in 0..USER_CODE_ATTEMPTS {
        let code = generate_user_code();
        if !cache.exists_user_code(&code).await? {
            return Ok(code);
        }
    }

    Err(Error::Unexpected(
        "could not generate an unused user code".into(),
    ))
}

#[derive(Debug, Clone)]
pub struct RegisterCmd {
    /// The `dmtr-gateway-token` metadata, if the caller sent one.
    pub gateway_token: Option<String>,
    pub agent_name: String,
    pub agent_version: Option<String>,
    pub agent_url: Option<String>,
    pub client_ip: String,
}
impl RegisterCmd {
    fn validate(&self) -> Result<()> {
        if self.agent_name.trim().is_empty() {
            return Err(Error::InvalidArgument("agent_name is required".into()));
        }
        if self.agent_name.chars().count() > AGENT_NAME_MAX {
            return Err(Error::InvalidArgument(format!(
                "agent_name exceeds {AGENT_NAME_MAX} characters"
            )));
        }
        for (field, value) in [
            ("agent_version", &self.agent_version),
            ("agent_url", &self.agent_url),
        ] {
            if value
                .as_ref()
                .is_some_and(|v| v.chars().count() > AGENT_FIELD_MAX)
            {
                return Err(Error::InvalidArgument(format!(
                    "{field} exceeds {AGENT_FIELD_MAX} characters"
                )));
            }
        }
        if self.client_ip.trim().is_empty() {
            return Err(Error::InvalidArgument("client_ip is required".into()));
        }

        Ok(())
    }
}

/// What `register` hands back to the caller, once. Not `Debug`, because it holds three secrets.
pub struct Registration {
    pub project_id: String,
    pub project_namespace: String,
    pub api_key: String,
    pub claim_token: String,
    /// The user code as a person types it, `XXXX-XXXX`.
    pub user_code: String,
    pub expires_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::{
        domain::{
            event::{Event, MockEventDrivenBridge},
            project::{
                self,
                cache::{MockProjectDrivenCache, ProjectDrivenCache},
                command::{verify_secret, VerifySecretCmd, PRE_CLAIM_SECRET_NAME},
                ProvisionalProject,
            },
            resource::{self, cache::ResourceDrivenCache, Resource, ResourceStatus},
            tests::SECRET,
        },
        driven::cache::{
            project::SqliteProjectDrivenCache, resource::SqliteResourceDrivenCache, SqliteCache,
        },
    };

    const GATEWAY_TOKEN: &str = "gateway token";
    const CLIENT_IP: &str = "203.0.113.7";

    fn config() -> ProvisionalConfig {
        ProvisionalConfig {
            enabled: true,
            gateway_token: GATEWAY_TOKEN.into(),
            ..Default::default()
        }
    }

    impl Default for RegisterCmd {
        fn default() -> Self {
            Self {
                gateway_token: Some(GATEWAY_TOKEN.into()),
                agent_name: "claude-code".into(),
                agent_version: Some("2.1.0".into()),
                agent_url: Some("https://claude.com/claude-code".into()),
                client_ip: CLIENT_IP.into(),
            }
        }
    }

    /// A bridge that projects every event into a real cache as it is dispatched, so a test sees
    /// the state the rpc would read once the projection caught up.
    struct ProjectingBridge {
        projects: Arc<dyn ProjectDrivenCache>,
        resources: Arc<dyn ResourceDrivenCache>,
        dispatched: Mutex<Vec<Event>>,
    }
    impl ProjectingBridge {
        fn keys(&self) -> Vec<String> {
            self.dispatched
                .lock()
                .unwrap()
                .iter()
                .map(Event::key)
                .collect()
        }
    }
    #[async_trait::async_trait]
    impl EventDrivenBridge for ProjectingBridge {
        async fn dispatch(&self, event: Event) -> Result<()> {
            match event.clone() {
                Event::ProvisionalProjectCreated(evt) => {
                    project::cache::create_provisional(self.projects.clone(), evt).await?
                }
                Event::ProjectSecretCreated(evt) => {
                    project::cache::create_secret(self.projects.clone(), evt).await?
                }
                Event::ProjectExpired(evt) => {
                    project::cache::expire(self.projects.clone(), evt).await?
                }
                Event::ResourceCreated(evt) => {
                    resource::cache::create(self.resources.clone(), evt).await?
                }
                other => panic!("unexpected event {}", other.key()),
            }
            self.dispatched.lock().unwrap().push(event);
            Ok(())
        }
    }

    async fn sqlite() -> (
        Arc<SqliteProjectDrivenCache>,
        Arc<SqliteResourceDrivenCache>,
        Arc<ProjectingBridge>,
    ) {
        let sqlite = Arc::new(SqliteCache::ephemeral().await.unwrap());
        let projects = Arc::new(SqliteProjectDrivenCache::new(sqlite.clone()));
        let resources = Arc::new(SqliteResourceDrivenCache::new(sqlite));
        let bridge = Arc::new(ProjectingBridge {
            projects: projects.clone(),
            resources: resources.clone(),
            dispatched: Mutex::new(Vec::new()),
        });
        (projects, resources, bridge)
    }

    /// Stores a provisional project directly, as an earlier registration would have.
    async fn seed(
        cache: &SqliteProjectDrivenCache,
        source_ip_hash: &str,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> ProvisionalProject {
        let evt = ProvisionalProjectCreated {
            namespace: utils::get_random_name(),
            user_code: generate_user_code(),
            source_ip_hash: source_ip_hash.into(),
            created_at,
            updated_at: created_at,
            expires_at,
            ..Default::default()
        };
        let provisional: ProvisionalProject = evt.try_into().unwrap();
        cache.create_provisional(&provisional).await.unwrap();
        provisional
    }

    async fn register_with(cmd: RegisterCmd, config: &ProvisionalConfig) -> Result<Registration> {
        // Mocks with no expectations: any cache read or dispatch fails the test.
        register(
            Arc::new(MockProjectDrivenCache::new()),
            Arc::new(MockEventDrivenBridge::new()),
            config,
            SECRET,
            cmd,
        )
        .await
    }

    #[tokio::test]
    async fn it_should_refuse_register_without_a_gateway_token() {
        let cmd = RegisterCmd {
            gateway_token: None,
            ..Default::default()
        };

        let result = register_with(cmd, &config()).await;
        assert!(matches!(result, Err(Error::Unauthenticated(_))));
    }

    #[tokio::test]
    async fn it_should_refuse_register_with_a_wrong_gateway_token() {
        let cmd = RegisterCmd {
            gateway_token: Some("wrong token".into()),
            ..Default::default()
        };

        let result = register_with(cmd, &config()).await;
        assert!(matches!(result, Err(Error::Unauthenticated(_))));
    }

    #[tokio::test]
    async fn it_should_refuse_register_when_no_gateway_token_is_configured() {
        let config = ProvisionalConfig {
            gateway_token: String::new(),
            ..config()
        };
        let cmd = RegisterCmd {
            gateway_token: Some(String::new()),
            ..Default::default()
        };

        let result = register_with(cmd, &config).await;
        assert!(matches!(result, Err(Error::Unauthenticated(_))));
    }

    #[tokio::test]
    async fn it_should_refuse_register_when_the_kill_switch_is_off() {
        let config = ProvisionalConfig {
            enabled: false,
            ..config()
        };

        let result = register_with(RegisterCmd::default(), &config).await;
        assert!(matches!(
            result,
            Err(Error::Unavailable(code)) if code == REGISTRATION_DISABLED
        ));
    }

    #[tokio::test]
    async fn it_should_keep_the_kill_switch_off_by_default() {
        let config = ProvisionalConfig {
            gateway_token: GATEWAY_TOKEN.into(),
            ..Default::default()
        };

        let result = register_with(RegisterCmd::default(), &config).await;
        assert!(matches!(
            result,
            Err(Error::Unavailable(code)) if code == REGISTRATION_DISABLED
        ));
    }

    #[tokio::test]
    async fn it_should_refuse_register_with_an_invalid_agent() {
        let cases = [
            RegisterCmd {
                agent_name: "  ".into(),
                ..Default::default()
            },
            RegisterCmd {
                agent_name: "a".repeat(AGENT_NAME_MAX + 1),
                ..Default::default()
            },
            RegisterCmd {
                agent_version: Some("v".repeat(AGENT_FIELD_MAX + 1)),
                ..Default::default()
            },
            RegisterCmd {
                agent_url: Some("u".repeat(AGENT_FIELD_MAX + 1)),
                ..Default::default()
            },
            RegisterCmd {
                client_ip: String::new(),
                ..Default::default()
            },
        ];

        for cmd in cases {
            let result = register_with(cmd, &config()).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))));
        }
    }

    #[tokio::test]
    async fn it_should_register_a_provisional_project() {
        let (projects, _, bridge) = sqlite().await;

        let registration = register(
            projects.clone(),
            bridge.clone(),
            &config(),
            SECRET,
            RegisterCmd::default(),
        )
        .await
        .unwrap();

        assert_eq!(
            bridge.keys(),
            vec!["ProvisionalProjectCreated", "ProjectSecretCreated"]
        );

        let project = projects
            .find_by_id(&registration.project_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.status, ProjectStatus::Provisional);
        assert_eq!(project.owner, None);
        assert_eq!(project.namespace, registration.project_namespace);
        assert_eq!(project.name, project.namespace);
        assert!(project.agent_provisioned);
        assert_eq!(project.agent_name.as_deref(), Some("claude-code"));
        assert_eq!(project.expires_at, Some(registration.expires_at));
        let ttl = registration.expires_at - project.created_at;
        assert_eq!(ttl, Duration::days(7));
        assert!(projects
            .find_users(&project.id, &1, &12)
            .await
            .unwrap()
            .is_empty());

        // The user code is live and is the one the response carries, without its hyphen.
        assert_eq!(registration.user_code.len(), 9);
        assert!(projects
            .exists_user_code(&registration.user_code.replace('-', ""))
            .await
            .unwrap());

        // The key is an ordinary project API key, named for what it is.
        let secret = verify_secret(
            projects.clone(),
            VerifySecretCmd {
                project: registration.project_id.clone(),
                key: registration.api_key.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(secret.project_id, registration.project_id);
        assert_eq!(secret.name, PRE_CLAIM_SECRET_NAME);
    }

    #[tokio::test]
    async fn it_should_store_only_the_hash_of_the_claim_token() {
        let (projects, _, bridge) = sqlite().await;

        let registration = register(
            projects,
            bridge.clone(),
            &config(),
            SECRET,
            RegisterCmd::default(),
        )
        .await
        .unwrap();

        let dispatched = bridge.dispatched.lock().unwrap();
        let Event::ProvisionalProjectCreated(evt) = &dispatched[0] else {
            panic!("the first event is not the project");
        };
        assert_eq!(
            evt.claim_token_hash,
            hash_claim_token(&registration.claim_token)
        );
        assert_ne!(evt.claim_token_hash, registration.claim_token);
        assert_ne!(evt.source_ip_hash, CLIENT_IP);
    }

    #[tokio::test]
    async fn it_should_hit_the_ip_cap_on_the_fourth_registration() {
        let (projects, _, bridge) = sqlite().await;
        let now = Utc::now();
        let hash = hash_source_ip(CLIENT_IP, SECRET);

        // One registration from the day before falls outside the trailing 24 hours.
        seed(&projects, &hash, now - Duration::hours(25), now).await;
        for _ in 0..3 {
            seed(&projects, &hash, now, now + Duration::days(7)).await;
        }

        let result = register(
            projects.clone(),
            bridge.clone(),
            &config(),
            SECRET,
            RegisterCmd::default(),
        )
        .await;
        assert!(matches!(
            &result,
            Err(Error::RetryAfter { message, retry_after_seconds })
                if message == IP_CAP && *retry_after_seconds > 23 * 3600
        ));
        assert!(bridge.keys().is_empty());

        let other = RegisterCmd {
            client_ip: "203.0.113.8".into(),
            ..Default::default()
        };
        let result = register(projects, bridge, &config(), SECRET, other).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_wait_for_the_oldest_registration_in_the_window_to_age_out() {
        let (projects, _, bridge) = sqlite().await;
        let now = Utc::now();
        let hash = hash_source_ip(CLIENT_IP, SECRET);
        seed(&projects, &hash, now - Duration::hours(25), now).await;
        seed(&projects, &hash, now - Duration::hours(23), now).await;
        seed(&projects, &hash, now - Duration::hours(1), now).await;
        seed(&projects, &hash, now - Duration::minutes(5), now).await;

        let result = register(projects, bridge, &config(), SECRET, RegisterCmd::default()).await;
        let Err(Error::RetryAfter {
            message,
            retry_after_seconds,
        }) = result
        else {
            panic!("the fourth registration is not refused with a retry after");
        };
        assert_eq!(message, IP_CAP);
        // The one from 23 hours ago leaves the window in an hour, less the test's own runtime.
        assert!((3500..=3600).contains(&retry_after_seconds));
    }

    #[tokio::test]
    async fn it_should_wait_past_a_burst_that_slipped_over_the_ip_cap() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_count_registrations_by_source()
            .return_once(|_, _| Ok(5));
        let now = Utc::now();
        cache
            .expect_find_registration_created_at_by_source()
            .withf(|_, _, offset| *offset == 2)
            .return_once(move |_, _, _| Ok(Some(now - Duration::hours(22))));

        let result = register(
            Arc::new(cache),
            Arc::new(MockEventDrivenBridge::new()),
            &config(),
            SECRET,
            RegisterCmd::default(),
        )
        .await;
        assert!(matches!(
            result,
            Err(Error::RetryAfter { retry_after_seconds, .. })
                if (7100..=7200).contains(&retry_after_seconds)
        ));
    }

    #[test]
    fn it_should_round_the_ip_cap_retry_after_up_to_whole_seconds() {
        let now = Utc::now();
        let window = IP_CAP_WINDOW;
        let at = |left: Duration| Some(now - window + left);

        assert_eq!(ip_cap_retry_after(at(Duration::hours(1)), now), 3600);
        assert_eq!(ip_cap_retry_after(at(Duration::milliseconds(3001)), now), 4);
        assert_eq!(ip_cap_retry_after(at(Duration::milliseconds(1)), now), 1);
        // Already out of the window, as at the edge of the query: retry at once, but not at zero.
        assert_eq!(ip_cap_retry_after(at(Duration::zero()), now), 1);
        assert_eq!(ip_cap_retry_after(at(-Duration::seconds(5)), now), 1);
        assert_eq!(ip_cap_retry_after(None, now), 86400);
    }

    #[tokio::test]
    async fn it_should_allow_three_registrations_from_one_ip() {
        let (projects, _, bridge) = sqlite().await;
        let now = Utc::now();
        let hash = hash_source_ip(CLIENT_IP, SECRET);
        for _ in 0..2 {
            seed(&projects, &hash, now, now + Duration::days(7)).await;
        }

        let result = register(projects, bridge, &config(), SECRET, RegisterCmd::default()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_hit_the_global_cap_at_200_live_projects() {
        let (projects, _, bridge) = sqlite().await;
        let now = Utc::now();
        for i in 0..199 {
            seed(
                &projects,
                &format!("hash {i}"),
                now,
                now + Duration::days(7),
            )
            .await;
        }

        let result = register(
            projects.clone(),
            bridge.clone(),
            &config(),
            SECRET,
            RegisterCmd::default(),
        )
        .await;
        assert!(result.is_ok(), "the 200th live project is allowed");

        let other = RegisterCmd {
            client_ip: "203.0.113.8".into(),
            ..Default::default()
        };
        let result = register(projects, bridge, &config(), SECRET, other).await;
        assert!(matches!(&result, Err(Error::Unavailable(code)) if code == GLOBAL_CAP));
    }

    #[tokio::test]
    async fn it_should_retry_the_user_code_on_collision() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_count_registrations_by_source()
            .return_once(|_, _| Ok(0));
        cache.expect_count_live_provisional().return_once(|| Ok(0));
        cache.expect_find_by_namespace().return_once(|_| Ok(None));
        let mut taken = 2;
        cache
            .expect_exists_user_code()
            .times(3)
            .returning(move |_| {
                taken -= 1;
                Ok(taken >= 0)
            });

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().times(2).returning(|_| Ok(()));

        let result = register(
            Arc::new(cache),
            Arc::new(event),
            &config(),
            SECRET,
            RegisterCmd::default(),
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_expire_only_projects_past_their_expiry() {
        let (projects, resources, bridge) = sqlite().await;
        let now = Utc::now();

        let past = seed(
            &projects,
            "a",
            now - Duration::days(8),
            now - Duration::hours(1),
        )
        .await;
        let live = seed(&projects, "b", now, now + Duration::days(7)).await;

        let resource = Resource {
            project_id: past.project.id.clone(),
            ..Default::default()
        };
        resources.create(&resource).await.unwrap();

        let expired = sweep_expired(projects.clone(), bridge.clone(), now)
            .await
            .unwrap();
        assert_eq!(expired, 1);
        assert_eq!(bridge.keys(), vec!["ProjectExpired"]);

        let past_project = projects
            .find_by_id(&past.project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(past_project.status, ProjectStatus::Expired);
        assert!(!projects.exists_user_code(&past.user_code).await.unwrap());
        let resource = resources.find_by_id(&resource.id).await.unwrap();
        assert!(resource.is_none(), "the expired project's port is deleted");

        let live_project = projects
            .find_by_id(&live.project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(live_project.status, ProjectStatus::Provisional);
        assert!(projects.exists_user_code(&live.user_code).await.unwrap());

        // The second sweep finds nothing to do and dispatches nothing.
        let expired = sweep_expired(projects, bridge.clone(), now).await.unwrap();
        assert_eq!(expired, 0);
        assert_eq!(bridge.keys(), vec!["ProjectExpired"]);
    }

    #[tokio::test]
    async fn it_should_ignore_a_duplicate_expiry_in_the_projection() {
        let (projects, resources, _) = sqlite().await;
        let now = Utc::now();
        let past = seed(
            &projects,
            "a",
            now - Duration::days(8),
            now - Duration::hours(1),
        )
        .await;

        let evt = ProjectExpired {
            id: past.project.id.clone(),
            namespace: past.project.namespace.clone(),
            expired_at: now,
        };
        project::cache::expire(projects.clone(), evt.clone())
            .await
            .unwrap();
        let first = projects
            .find_by_id(&past.project.id)
            .await
            .unwrap()
            .unwrap();

        // A port created after the expiry, as from a stale command, survives the duplicate.
        let resource = Resource {
            project_id: past.project.id.clone(),
            status: ResourceStatus::Active,
            ..Default::default()
        };
        resources.create(&resource).await.unwrap();

        let duplicate = ProjectExpired {
            expired_at: now + Duration::minutes(10),
            ..evt
        };
        project::cache::expire(projects.clone(), duplicate)
            .await
            .unwrap();

        let second = projects
            .find_by_id(&past.project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.status, ProjectStatus::Expired);
        assert_eq!(second.updated_at, first.updated_at);
        assert!(resources.find_by_id(&resource.id).await.unwrap().is_some());
    }
}
