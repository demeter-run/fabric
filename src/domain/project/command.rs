use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use bech32::{Bech32m, Hrp};
use chrono::Utc;
use rand::{
    distributions::{Alphanumeric, DistString},
    rngs::OsRng,
};
use std::{sync::Arc, time::Duration};
use tracing::{error, info};
use uuid::Uuid;

use crate::domain::{
    auth::{Auth0Driven, Credential, UserId},
    error::Error,
    event::{
        EventDrivenBridge, ProjectCreated, ProjectDeleted, ProjectSecretCreated, ProjectUpdated,
        ProjectUserDeleted, ProjectUserInviteAccepted, ProjectUserInviteCreated,
    },
    project::{ProjectStatus, ProjectUserInviteStatus},
    utils, Result, MAX_SECRET, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{
    cache::ProjectDrivenCache, Project, ProjectEmailDriven, ProjectSecret, ProjectUser,
    ProjectUserInvite, ProjectUserRole, StripeDriven,
};

pub async fn fetch(cache: Arc<dyn ProjectDrivenCache>, cmd: FetchCmd) -> Result<Vec<Project>> {
    let user_id = assert_credential(&cmd.credential)?;

    cache.find(&user_id, &cmd.page, &cmd.page_size).await
}

pub async fn create(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    auth0: Arc<dyn Auth0Driven>,
    stripe: Arc<dyn StripeDriven>,
    cmd: CreateCmd,
) -> Result<()> {
    let user_id = assert_credential(&cmd.credential)?;

    if cache.find_by_namespace(&cmd.namespace).await?.is_some() {
        return Err(Error::CommandMalformed("invalid project namespace".into()));
    }

    let (name, email) = auth0.find_info(&user_id).await?;
    let billing_provider_id = stripe.create_customer(&name, &email).await?;

    let evt = ProjectCreated {
        id: cmd.id,
        namespace: cmd.namespace.clone(),
        name: cmd.name,
        owner: user_id,
        status: ProjectStatus::Active.to_string(),
        billing_provider: "stripe".into(),
        billing_provider_id,
        billing_subscription_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(project = cmd.namespace, "new project created");

    Ok(())
}

pub async fn update(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: UpdateCmd,
) -> Result<Project> {
    assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.id, None).await?;

    let evt = ProjectUpdated {
        id: cmd.id.clone(),
        name: Some(cmd.name.clone()),
        status: None,
        updated_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(project = &cmd.id, "project updated");

    let Some(project) = cache.find_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("Missing project".into()));
    };

    Ok(project)
}

pub async fn delete(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: DeleteCmd,
) -> Result<()> {
    assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.id, None).await?;

    let project = match cache.find_by_id(&cmd.id).await? {
        Some(project) => project,
        None => return Err(Error::Unexpected("Failed to locate project.".into())),
    };

    let evt = ProjectDeleted {
        id: cmd.id.clone(),
        namespace: project.namespace,
        deleted_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(project = &cmd.id, "project updated");

    Ok(())
}

pub async fn fetch_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    cmd: FetchSecretCmd,
) -> Result<Vec<ProjectSecret>> {
    assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.project_id, None).await?;

    cache.find_secrets(&cmd.project_id).await
}

pub async fn create_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateSecretCmd,
) -> Result<String> {
    assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.project_id, None).await?;

    let Some(project) = cache.find_by_id(&cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let secrets = cache.find_secrets(&cmd.project_id).await?;
    if secrets.len() >= MAX_SECRET {
        return Err(Error::SecretExceeded(format!(
            "secrets exceeded the limit of {MAX_SECRET}"
        )));
    }

    let key = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    let salt_string = SaltString::generate(&mut OsRng);
    let secret = cmd.secret.into_bytes();

    let argon2 = match Argon2::new_with_secret(
        &secret,
        Default::default(),
        Default::default(),
        Default::default(),
    ) {
        Ok(argon2) => argon2.clone(),
        Err(error) => {
            error!(?error, "error to configure argon2 with secret");
            return Err(Error::Unexpected("error to create the secret".into()));
        }
    };

    let key_bytes = key.into_bytes();

    let password_hash = argon2.hash_password(&key_bytes, salt_string.as_salt())?;

    let hrp = Hrp::parse("dmtr_apikey")?;
    let key = bech32::encode::<Bech32m>(hrp, &key_bytes)?;

    let evt = ProjectSecretCreated {
        id: cmd.id,
        project_id: project.id,
        name: cmd.name,
        phc: password_hash.to_string(),
        secret: secret.to_vec(),
        created_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!("new project secret created");

    Ok(key)
}

pub async fn verify_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    cmd: VerifySecretCmd,
) -> Result<ProjectSecret> {
    let (hrp, key) = bech32::decode(&cmd.key).map_err(|error| {
        error!(?error, "invalid bech32");
        Error::Unauthorized("invalid bech32".into())
    })?;

    if !hrp.to_string().eq("dmtr_apikey") {
        error!(?hrp, "invalid bech32 hrp");
        return Err(Error::Unauthorized("invalid project secret".into()));
    }

    let secrets = cache.find_secrets(&cmd.project_id).await?;

    let secret = secrets.into_iter().find(|project_secret| {
        let argon2 = Argon2::new_with_secret(
            &project_secret.secret,
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .unwrap();

        let Ok(password_hash) = PasswordHash::new(&project_secret.phc) else {
            error!(
                project_id = cmd.project_id,
                secret_id = project_secret.id,
                "error to decode phc"
            );
            return false;
        };

        argon2.verify_password(&key, &password_hash).is_ok()
    });

    let Some(secret) = secret else {
        return Err(Error::Unauthorized("invalid project secret".into()));
    };

    Ok(secret)
}

pub async fn fetch_user(
    cache: Arc<dyn ProjectDrivenCache>,
    cmd: FetchUserCmd,
) -> Result<Vec<ProjectUser>> {
    assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.project_id, None).await?;

    cache
        .find_users(&cmd.project_id, &cmd.page, &cmd.page_size)
        .await
}
pub async fn delete_user(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: DeleteUserCmd,
) -> Result<()> {
    let user_id = assert_credential(&cmd.credential)?;
    assert_permission(
        cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

    let Some(project) = cache.find_by_id(&cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    if project.owner == cmd.id {
        return Err(Error::CommandMalformed("owner can not be deleted".into()));
    }

    let Some(user_permission) = cache.find_user_permission(&cmd.id, &cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid user id".into()));
    };

    let evt = ProjectUserDeleted {
        id: Uuid::new_v4().to_string(),
        project_id: project.id,
        user_id: user_permission.user_id,
        role: user_permission.role.to_string(),
        deleted_by: user_id,
        deleted_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(user = &cmd.id, "project user deleted");

    Ok(())
}

pub async fn fetch_user_invite(
    cache: Arc<dyn ProjectDrivenCache>,
    cmd: FetchUserInviteCmd,
) -> Result<Vec<ProjectUserInvite>> {
    assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.project_id, None).await?;

    cache
        .find_user_invites(&cmd.project_id, &cmd.page, &cmd.page_size)
        .await
}

pub async fn create_user_invite(
    cache: Arc<dyn ProjectDrivenCache>,
    email: Arc<dyn ProjectEmailDriven>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateUserInviteCmd,
) -> Result<()> {
    assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.project_id, None).await?;

    let Some(project) = cache.find_by_id(&cmd.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let code = Uuid::new_v4().to_string();

    let expires_in = Utc::now() + cmd.ttl;

    email
        .send_invite(&project.name, &cmd.email, &code, &expires_in)
        .await?;

    let evt = ProjectUserInviteCreated {
        id: cmd.id,
        project_id: project.id,
        email: cmd.email,
        role: cmd.role.to_string(),
        code,
        expires_in,
        created_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(?expires_in, "new project invite created");

    Ok(())
}

pub async fn accept_user_invite(
    cache: Arc<dyn ProjectDrivenCache>,
    auth0: Arc<dyn Auth0Driven>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: AcceptUserInviteCmd,
) -> Result<()> {
    let user_id = assert_credential(&cmd.credential)?;

    let Some(user_invite) = cache.find_user_invite_by_code(&cmd.code).await? else {
        return Err(Error::CommandMalformed("invalid invite code".into()));
    };
    if Utc::now() > user_invite.expires_in {
        return Err(Error::CommandMalformed("invite code expired".into()));
    }

    if cache
        .find_user_permission(&user_id, &user_invite.project_id)
        .await?
        .is_some()
    {
        return Err(Error::CommandMalformed(
            "user already is in the project".into(),
        ));
    };

    if user_invite.status != ProjectUserInviteStatus::Sent {
        return Err(Error::CommandMalformed(
            "invite is not available anymore".into(),
        ));
    }

    let (_, email) = auth0.find_info(&user_id).await?;
    if user_invite.email != email {
        return Err(Error::CommandMalformed(
            "user email doesnt match with invite".into(),
        ));
    }

    let evt = ProjectUserInviteAccepted {
        id: cmd.id,
        project_id: user_invite.project_id,
        user_id,
        role: user_invite.role.to_string(),
        created_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!("new project invite accepted");

    Ok(())
}

pub async fn resend_user_invite(
    cache: Arc<dyn ProjectDrivenCache>,
    email: Arc<dyn ProjectEmailDriven>,
    cmd: ResendUserInviteCmd,
) -> Result<()> {
    assert_credential(&cmd.credential)?;

    let Some(user_invite) = cache.find_user_invite_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("invalid invite id".into()));
    };

    assert_permission(
        cache.clone(),
        &cmd.credential,
        &user_invite.project_id,
        None,
    )
    .await?;

    if Utc::now() > user_invite.expires_in {
        return Err(Error::CommandMalformed("invite code expired".into()));
    }

    let Some(project) = cache.find_by_id(&user_invite.project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    email
        .send_invite(
            &project.name,
            &user_invite.email,
            &user_invite.code,
            &user_invite.expires_in,
        )
        .await?;

    info!("project invite resent");

    Ok(())
}

fn assert_credential(credential: &Credential) -> Result<UserId> {
    match credential {
        Credential::Auth0(user_id) => Ok(user_id.into()),
        Credential::ApiKey(_) => Err(Error::Unauthorized(
            "project rpc doesnt support secret".into(),
        )),
    }
}
async fn assert_permission(
    cache: Arc<dyn ProjectDrivenCache>,
    credential: &Credential,
    project_id: &str,
    role: Option<ProjectUserRole>,
) -> Result<()> {
    match credential {
        Credential::Auth0(user_id) => {
            let result = cache.find_user_permission(user_id, project_id).await?;
            if result.is_none() {
                return Err(Error::Unauthorized("user doesnt have permission".into()));
            }

            match role {
                Some(role) => {
                    let permission = result.unwrap();
                    if role != permission.role {
                        return Err(Error::Unauthorized("user doesnt have permission".into()));
                    }
                    Ok(())
                }
                None => Ok(()),
            }
        }
        Credential::ApiKey(_) => Err(Error::Unauthorized("rpc doesnt support api-key".into())),
    }
}

#[derive(Debug, Clone)]
pub struct FetchCmd {
    pub credential: Credential,
    pub page: u32,
    pub page_size: u32,
}
impl FetchCmd {
    pub fn new(credential: Credential, page: Option<u32>, page_size: Option<u32>) -> Result<Self> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(PAGE_SIZE_DEFAULT);

        if page_size >= PAGE_SIZE_MAX {
            return Err(Error::CommandMalformed(format!(
                "page_size exceeded the limit of {PAGE_SIZE_MAX}"
            )));
        }

        Ok(Self {
            credential,
            page,
            page_size,
        })
    }
}
#[derive(Debug, Clone)]
pub struct CreateCmd {
    pub credential: Credential,
    pub id: String,
    pub name: String,
    pub namespace: String,
}
impl CreateCmd {
    pub fn new(credential: Credential, name: String) -> Self {
        let id = Uuid::new_v4().to_string();
        let namespace = format!("prj-{}", utils::get_random_name());

        Self {
            credential,
            id,
            name,
            namespace,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateCmd {
    pub credential: Credential,
    pub id: String,
    pub name: String,
}
impl UpdateCmd {
    pub fn new(credential: Credential, id: String, name: String) -> Self {
        Self {
            credential,
            id,
            name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeleteCmd {
    pub credential: Credential,
    pub id: String,
}
impl DeleteCmd {
    pub fn new(credential: Credential, id: String) -> Self {
        Self { credential, id }
    }
}

#[derive(Debug, Clone)]
pub struct FetchSecretCmd {
    pub credential: Credential,
    pub project_id: String,
}
impl FetchSecretCmd {
    pub fn new(credential: Credential, project_id: String) -> Self {
        Self {
            credential,
            project_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateSecretCmd {
    pub credential: Credential,
    pub secret: String,
    pub id: String,
    pub project_id: String,
    pub name: String,
}
impl CreateSecretCmd {
    pub fn new(credential: Credential, secret: String, project_id: String, name: String) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            credential,
            secret,
            id,
            project_id,
            name,
        }
    }
}
#[derive(Debug, Clone)]
pub struct VerifySecretCmd {
    pub project_id: String,
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct FetchUserCmd {
    pub credential: Credential,
    pub page: u32,
    pub page_size: u32,
    pub project_id: String,
}
impl FetchUserCmd {
    pub fn new(
        credential: Credential,
        page: Option<u32>,
        page_size: Option<u32>,
        project_id: String,
    ) -> Result<Self> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(PAGE_SIZE_DEFAULT);

        if page_size >= PAGE_SIZE_MAX {
            return Err(Error::CommandMalformed(format!(
                "page_size exceeded the limit of {PAGE_SIZE_MAX}"
            )));
        }

        Ok(Self {
            credential,
            page,
            page_size,
            project_id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DeleteUserCmd {
    pub credential: Credential,
    pub project_id: String,
    pub id: String,
}
impl DeleteUserCmd {
    pub fn new(credential: Credential, project_id: String, id: String) -> Self {
        Self {
            credential,
            project_id,
            id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchUserInviteCmd {
    pub credential: Credential,
    pub page: u32,
    pub page_size: u32,
    pub project_id: String,
}
impl FetchUserInviteCmd {
    pub fn new(
        credential: Credential,
        page: Option<u32>,
        page_size: Option<u32>,
        project_id: String,
    ) -> Result<Self> {
        let page = page.unwrap_or(1);
        let page_size = page_size.unwrap_or(PAGE_SIZE_DEFAULT);

        if page_size >= PAGE_SIZE_MAX {
            return Err(Error::CommandMalformed(format!(
                "page_size exceeded the limit of {PAGE_SIZE_MAX}"
            )));
        }

        Ok(Self {
            credential,
            page,
            page_size,
            project_id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CreateUserInviteCmd {
    pub credential: Credential,
    pub ttl: Duration,
    pub id: String,
    pub project_id: String,
    pub email: String,
    pub role: ProjectUserRole,
}
impl CreateUserInviteCmd {
    pub fn try_new(
        credential: Credential,
        ttl: Duration,
        project_id: String,
        email: String,
        role: ProjectUserRole,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();

        Ok(Self {
            credential,
            ttl,
            id,
            project_id,
            email,
            role,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AcceptUserInviteCmd {
    pub credential: Credential,
    pub id: String,
    pub code: String,
}
impl AcceptUserInviteCmd {
    pub fn new(credential: Credential, code: String) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            credential,
            id,
            code,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResendUserInviteCmd {
    pub credential: Credential,
    pub id: String,
}
impl ResendUserInviteCmd {
    pub fn new(credential: Credential, id: String) -> Self {
        Self { credential, id }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        auth::MockAuth0Driven,
        event::MockEventDrivenBridge,
        project::{
            cache::MockProjectDrivenCache, MockProjectEmailDriven, MockStripeDriven, ProjectUser,
            ProjectUserInvite,
        },
        tests::{INVALID_HRP_KEY, INVALID_KEY, KEY, SECRET},
    };

    impl Default for FetchCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                page: 1,
                page_size: 12,
            }
        }
    }
    impl Default for CreateCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                name: "New Project".into(),
                namespace: "sonic-vegas".into(),
            }
        }
    }
    impl Default for UpdateCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                name: "Other name".into(),
            }
        }
    }
    impl Default for FetchSecretCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                project_id: Uuid::new_v4().to_string(),
            }
        }
    }
    impl Default for CreateSecretCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                name: "Key 1".into(),
                secret: SECRET.into(),
            }
        }
    }
    impl Default for VerifySecretCmd {
        fn default() -> Self {
            Self {
                project_id: Default::default(),
                key: KEY.into(),
            }
        }
    }
    impl Default for FetchUserInviteCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                page: 1,
                page_size: 12,
                project_id: Uuid::new_v4().to_string(),
            }
        }
    }
    impl Default for CreateUserInviteCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                ttl: Duration::from_secs(15 * 60),
                id: Uuid::new_v4().to_string(),
                project_id: Uuid::new_v4().to_string(),
                email: "p@txpipe.io".into(),
                role: ProjectUserRole::Owner,
            }
        }
    }
    impl Default for AcceptUserInviteCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
                code: "123".into(),
            }
        }
    }
    impl Default for ResendUserInviteCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
            }
        }
    }
    impl Default for FetchUserCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                page: 1,
                page_size: 12,
                project_id: Uuid::new_v4().to_string(),
            }
        }
    }
    impl Default for DeleteUserCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: "member user id".into(),
                project_id: Uuid::new_v4().to_string(),
            }
        }
    }

    #[tokio::test]
    async fn it_should_fetch_user_projects() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find()
            .return_once(|_, _, _| Ok(vec![Project::default()]));

        let cmd = FetchCmd::default();

        let result = fetch(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_create_project() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_by_namespace().return_once(|_| Ok(None));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(("user name".into(), "user email".into())));

        let mut stripe = MockStripeDriven::new();
        stripe
            .expect_create_customer()
            .return_once(|_, _| Ok("stripe id".into()));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            Arc::new(stripe),
            cmd,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_fail_create_project_when_namespace_exists() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_by_namespace()
            .return_once(|_| Ok(Some(Project::default())));

        let auth0 = MockAuth0Driven::new();
        let stripe = MockStripeDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateCmd::default();

        let result = create(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            Arc::new(stripe),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_when_invalid_permission() {
        let cache = MockProjectDrivenCache::new();
        let auth0 = MockAuth0Driven::new();
        let stripe = MockStripeDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateCmd {
            credential: Credential::ApiKey("xxxx".into()),
            ..Default::default()
        };

        let result = create(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            Arc::new(stripe),
            cmd,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_update_project() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));
        cache.expect_find_secrets().return_once(|_| Ok(Vec::new()));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = UpdateCmd::default();

        let result = update(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_fetch_project_secrets() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_secrets()
            .return_once(|_| Ok(vec![ProjectSecret::default()]));

        let cmd = FetchSecretCmd::default();

        let result = fetch_secret(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_create_project_secret() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));
        cache.expect_find_secrets().return_once(|_| Ok(Vec::new()));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_project_doesnt_exists() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache.expect_find_by_id().return_once(|_| Ok(None));

        let event = MockEventDrivenBridge::new();

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_invalid_credential() {
        let cache = MockProjectDrivenCache::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateSecretCmd {
            credential: Credential::ApiKey("xxxx".into()),
            ..Default::default()
        };

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_invalid_permission() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let event = MockEventDrivenBridge::new();

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_secret_when_max_secret_exceeded() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));
        cache
            .expect_find_secrets()
            .return_once(|_| Ok(vec![ProjectSecret::default(); 3]));

        let event = MockEventDrivenBridge::new();

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_verify_secret() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_secrets()
            .return_once(|_| Ok(vec![ProjectSecret::default()]));

        let cmd = VerifySecretCmd::default();

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_key() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_secrets()
            .return_once(|_| Ok(vec![ProjectSecret::default()]));

        let cmd = VerifySecretCmd {
            key: INVALID_KEY.into(),
            ..Default::default()
        };

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_bech32() {
        let cache = MockProjectDrivenCache::new();

        let cmd = VerifySecretCmd {
            key: "invalid bech32".into(),
            ..Default::default()
        };

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_invalid_bech32_hrp() {
        let cache = MockProjectDrivenCache::new();

        let cmd = VerifySecretCmd {
            key: INVALID_HRP_KEY.into(),
            ..Default::default()
        };

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_verify_secret_when_there_arent_secrets_storaged() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_secrets().return_once(|_| Ok(vec![]));

        let cmd = VerifySecretCmd::default();

        let result = verify_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_fetch_project_user_invites() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_user_invites()
            .return_once(|_, _, _| Ok(vec![ProjectUserInvite::default()]));

        let cmd = FetchUserInviteCmd::default();

        let result = fetch_user_invite(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_create_project_user_invite() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut email = MockProjectEmailDriven::new();
        email.expect_send_invite().return_once(|_, _, _, _| Ok(()));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = CreateUserInviteCmd::default();

        let result =
            create_user_invite(Arc::new(cache), Arc::new(email), Arc::new(event), cmd).await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_user_invite_when_project_doesnt_exists() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache.expect_find_by_id().return_once(|_| Ok(None));

        let email = MockProjectEmailDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateUserInviteCmd::default();

        let result =
            create_user_invite(Arc::new(cache), Arc::new(email), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_user_invite_when_invalid_credential() {
        let cache = MockProjectDrivenCache::new();
        let email = MockProjectEmailDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateUserInviteCmd {
            credential: Credential::ApiKey("xxxx".into()),
            ..Default::default()
        };

        let result =
            create_user_invite(Arc::new(cache), Arc::new(email), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_user_invite_when_invalid_permission() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let email = MockProjectEmailDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateUserInviteCmd::default();

        let result =
            create_user_invite(Arc::new(cache), Arc::new(email), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_accept_project_user_invite() {
        let invite = ProjectUserInvite::default();
        let invite_email = invite.email.clone();

        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_invite_by_code()
            .return_once(|_| Ok(Some(invite)));
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(("user name".into(), invite_email)));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = AcceptUserInviteCmd::default();

        let result =
            accept_user_invite(Arc::new(cache), Arc::new(auth0), Arc::new(event), cmd).await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_accept_project_user_invite_when_invalid_code() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_invite_by_code()
            .return_once(|_| Ok(None));

        let auth0 = MockAuth0Driven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = AcceptUserInviteCmd::default();

        let result =
            accept_user_invite(Arc::new(cache), Arc::new(auth0), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_accept_project_user_invite_when_invalid_credential() {
        let cache = MockProjectDrivenCache::new();
        let auth0 = MockAuth0Driven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = AcceptUserInviteCmd {
            credential: Credential::ApiKey("xxxx".into()),
            ..Default::default()
        };

        let result =
            accept_user_invite(Arc::new(cache), Arc::new(auth0), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_accept_project_user_invite_when_email_doesnt_match() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_invite_by_code()
            .return_once(|_| Ok(Some(ProjectUserInvite::default())));
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(("user name".into(), "user email".into())));

        let event = MockEventDrivenBridge::new();

        let cmd = AcceptUserInviteCmd::default();

        let result =
            accept_user_invite(Arc::new(cache), Arc::new(auth0), Arc::new(event), cmd).await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_accept_project_user_invite_when_invite_has_already_been_accepted() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_invite_by_code().return_once(|_| {
            Ok(Some(ProjectUserInvite {
                status: ProjectUserInviteStatus::Accepted,
                ..Default::default()
            }))
        });
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let auth0 = MockAuth0Driven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = AcceptUserInviteCmd::default();

        let result =
            accept_user_invite(Arc::new(cache), Arc::new(auth0), Arc::new(event), cmd).await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_accept_project_user_invite_when_user_already_is_in_the_project() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_invite_by_code().return_once(|_| {
            Ok(Some(ProjectUserInvite {
                status: ProjectUserInviteStatus::Accepted,
                ..Default::default()
            }))
        });
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let auth0 = MockAuth0Driven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = AcceptUserInviteCmd::default();

        let result =
            accept_user_invite(Arc::new(cache), Arc::new(auth0), Arc::new(event), cmd).await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_accept_project_user_invite_when_invite_code_expired() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_invite_by_code().return_once(|_| {
            Ok(Some(ProjectUserInvite {
                expires_in: Utc::now() - Duration::from_secs(10),
                ..Default::default()
            }))
        });

        let auth0 = MockAuth0Driven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = AcceptUserInviteCmd::default();

        let result =
            accept_user_invite(Arc::new(cache), Arc::new(auth0), Arc::new(event), cmd).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_resend_project_user_invite() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_user_invite_by_id()
            .return_once(|_| Ok(Some(ProjectUserInvite::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut email = MockProjectEmailDriven::new();
        email.expect_send_invite().return_once(|_, _, _, _| Ok(()));

        let cmd = ResendUserInviteCmd::default();

        let result = resend_user_invite(Arc::new(cache), Arc::new(email), cmd).await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_resend_project_user_invite_when_invite_doesnt_exist() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_user_invite_by_id()
            .return_once(|_| Ok(None));

        let email = MockProjectEmailDriven::new();

        let cmd = ResendUserInviteCmd::default();

        let result = resend_user_invite(Arc::new(cache), Arc::new(email), cmd).await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_resend_project_user_invite_when_invite_code_expired() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache.expect_find_user_invite_by_id().return_once(|_| {
            Ok(Some(ProjectUserInvite {
                expires_in: Utc::now() - Duration::from_secs(10),
                ..Default::default()
            }))
        });

        let email = MockProjectEmailDriven::new();

        let cmd = ResendUserInviteCmd::default();

        let result = resend_user_invite(Arc::new(cache), Arc::new(email), cmd).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_fetch_project_users() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_users()
            .return_once(|_, _, _| Ok(vec![ProjectUser::default()]));

        let cmd = FetchUserCmd::default();

        let result = fetch_user(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_delete_project_user() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .times(2)
            .returning(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = DeleteUserCmd::default();

        let result = delete_user(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_should_fail_delete_project_user_when_the_project_owner_is_the_user() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = DeleteUserCmd {
            id: "user id".into(),
            ..Default::default()
        };

        let result = delete_user(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }
}
