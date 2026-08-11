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
    auth::{assert_permission, Auth0Driven, Credential, UserId},
    error::Error,
    event::{
        EventDrivenBridge, ProjectCreated, ProjectDeleted, ProjectOwnerChanged,
        ProjectSecretCreated, ProjectSecretDeleted, ProjectUpdated, ProjectUserDeleted,
        ProjectUserInviteAccepted, ProjectUserInviteCreated, ProjectUserInviteDeleted,
    },
    project::{ProjectStatus, ProjectUserAggregated, ProjectUserInviteStatus},
    utils, Result, MAX_SECRET, PAGE_SIZE_DEFAULT, PAGE_SIZE_MAX,
};

use super::{
    cache::ProjectDrivenCache, Project, ProjectEmailDriven, ProjectSecret, ProjectUserInvite,
    ProjectUserRole, StripeDriven,
};

pub async fn fetch(cache: Arc<dyn ProjectDrivenCache>, cmd: FetchCmd) -> Result<Vec<Project>> {
    let user_id = assert_credential(&cmd.credential)?;

    cache.find(&user_id, &cmd.page, &cmd.page_size).await
}

pub async fn fetch_by_id(cache: Arc<dyn ProjectDrivenCache>, cmd: FetchByIdCmd) -> Result<Project> {
    let Some(project) = cache.find_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };
    assert_permission(cache.clone(), &cmd.credential, &project.id, None).await?;

    Ok(project)
}

pub async fn fetch_by_namespace(
    cache: Arc<dyn ProjectDrivenCache>,
    cmd: FetchByNamespaceCmd,
) -> Result<Project> {
    let Some(project) = cache.find_by_namespace(&cmd.namespace).await? else {
        return Err(Error::CommandMalformed("invalid project namespace".into()));
    };
    assert_permission(cache.clone(), &cmd.credential, &project.id, None).await?;

    Ok(project)
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

    let profile = auth0.find_info(&format!("user_id:{user_id}")).await?;
    if profile.is_empty() {
        return Err(Error::Unexpected("Invalid user_id".into()));
    }
    let profile = profile.first().unwrap();

    let billing_provider_id = stripe
        .create_customer(&profile.name, &profile.email)
        .await?;

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
    assert_permission(
        cache.clone(),
        &cmd.credential,
        &cmd.id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

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

/// Self-service entry point for an ownership transfer: only the current owner may transfer.
///
/// Not yet reachable over gRPC — transfers are run from the backoffice CLI, which calls
/// `apply_ownership_transfer` directly. This exists so the authorization rule for the self-service
/// path is settled and tested ahead of the RPC being added.
#[allow(dead_code)]
pub async fn transfer_ownership(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    auth0: Arc<dyn Auth0Driven>,
    stripe: Arc<dyn StripeDriven>,
    cmd: TransferOwnershipCmd,
) -> Result<()> {
    let user_id = assert_credential(&cmd.credential)?;
    assert_permission(
        cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

    apply_ownership_transfer(
        cache,
        event,
        auth0,
        Some(stripe),
        &cmd.project_id,
        &cmd.new_owner,
        &user_id,
        false,
    )
    .await
}

/// Validates and performs an ownership transfer, with no authorization check of its own.
///
/// Callers are responsible for authorization: `transfer_ownership` asserts the caller holds the
/// `Owner` role, while the backoffice driver is already privileged and passes `changed_by` to
/// record who ran it.
///
/// With `dry_run`, every validation still runs but neither the Stripe update nor the event
/// dispatch happens, so an operator can confirm a transfer is viable before committing to it.
///
/// `stripe` is `None` when the caller wants the billing contact left alone. The customer's `name`
/// and `email` are overwritten wholesale when it is `Some`, which destroys a billing address that
/// was curated rather than tracking the owner — so an operator who knows that is the case passes
/// `None`. `billing_provider_id` is never touched either way, so billing continuity is unaffected.
#[allow(clippy::too_many_arguments)]
pub async fn apply_ownership_transfer(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    auth0: Arc<dyn Auth0Driven>,
    stripe: Option<Arc<dyn StripeDriven>>,
    project_id: &str,
    new_owner: &str,
    changed_by: &str,
    dry_run: bool,
) -> Result<()> {
    let Some(project) = cache.find_by_id(project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    if project.owner == new_owner {
        return Err(Error::CommandMalformed(
            "user already is the project owner".into(),
        ));
    }

    if cache
        .find_user_permission(new_owner, project_id)
        .await?
        .is_none()
    {
        return Err(Error::CommandMalformed(
            "new owner is not a member of the project".into(),
        ));
    }

    let profile = auth0.find_info(&format!("user_id:{new_owner}")).await?;
    if profile.is_empty() {
        return Err(Error::Unexpected("Invalid user_id".into()));
    }
    let profile = profile.first().unwrap();

    let evt = ProjectOwnerChanged {
        id: Uuid::new_v4().to_string(),
        project_id: project.id,
        previous_owner: project.owner,
        new_owner: new_owner.to_string(),
        changed_by: changed_by.to_string(),
        changed_at: Utc::now(),
    };

    if dry_run {
        info!("event to dispath: {:?}", evt);
        match &stripe {
            Some(_) => info!(
                stripe_customer = &project.billing_provider_id,
                name = &profile.name,
                email = &profile.email,
                "stripe customer to update"
            ),
            None => info!(
                stripe_customer = &project.billing_provider_id,
                "stripe customer left unchanged"
            ),
        }
        return Ok(());
    }

    // The stripe call happens here, before dispatching, because projections are replayed in full
    // whenever the cache is rebuilt. See `create` above for the same pattern.
    match &stripe {
        Some(stripe) => {
            stripe
                .update_customer(&project.billing_provider_id, &profile.name, &profile.email)
                .await?
        }
        None => info!(
            stripe_customer = &project.billing_provider_id,
            "stripe customer left unchanged"
        ),
    }

    event.dispatch(evt.into()).await?;
    info!(project = project_id, "project owner changed");

    Ok(())
}

pub async fn delete(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: DeleteCmd,
) -> Result<()> {
    let user_id = assert_credential(&cmd.credential)?;
    assert_permission(
        cache.clone(),
        &cmd.credential,
        &cmd.id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

    let project = match cache.find_by_id(&cmd.id).await? {
        Some(project) => project,
        None => return Err(Error::Unexpected("Failed to locate project.".into())),
    };

    if user_id != project.owner {
        return Err(Error::CommandMalformed(
            "Just the creator can delete the project".into(),
        ));
    }

    let evt = ProjectDeleted {
        id: cmd.id.clone(),
        namespace: project.namespace,
        deleted_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(project = &cmd.id, "project deleted");

    Ok(())
}

pub async fn fetch_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    cmd: FetchSecretCmd,
) -> Result<Vec<ProjectSecret>> {
    assert_credential(&cmd.credential)?;
    assert_permission(
        cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

    cache.find_secrets(&cmd.project_id).await
}

pub async fn create_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: CreateSecretCmd,
) -> Result<String> {
    assert_credential(&cmd.credential)?;
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

    let secrets = cache.find_secrets(&cmd.project).await?;

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
                project = cmd.project,
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
pub async fn delete_secret(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: DeleteSecretCmd,
) -> Result<()> {
    let user_id = assert_credential(&cmd.credential)?;

    let Some(secret) = cache.find_secret_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("invalid secret id".into()));
    };

    assert_permission(
        cache.clone(),
        &cmd.credential,
        &secret.project_id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

    let evt = ProjectSecretDeleted {
        id: secret.id,
        deleted_by: user_id,
        deleted_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!(secret = &cmd.id, "project secret deleted");

    Ok(())
}

pub async fn fetch_user(
    cache: Arc<dyn ProjectDrivenCache>,
    auth0: Arc<dyn Auth0Driven>,
    cmd: FetchUserCmd,
) -> Result<Vec<ProjectUserAggregated>> {
    assert_credential(&cmd.credential)?;
    assert_permission(
        cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

    list_users(cache, auth0, &cmd.project_id, &cmd.page, &cmd.page_size).await
}

/// Lists a project's users with their roles, enriched with their Auth0 profiles.
///
/// No authorization check of its own — `fetch_user` asserts the caller holds the `Owner` role
/// before delegating here, and the backoffice driver is already privileged.
pub async fn list_users(
    cache: Arc<dyn ProjectDrivenCache>,
    auth0: Arc<dyn Auth0Driven>,
    project_id: &str,
    page: &u32,
    page_size: &u32,
) -> Result<Vec<ProjectUserAggregated>> {
    let project_users = cache.find_users(project_id, page, page_size).await?;

    // An empty page would otherwise build an empty Auth0 query, which is not a search for nothing
    // — it returns every user in the tenant.
    if project_users.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<String> = project_users
        .iter()
        .map(|p| format!("user_id:{}", p.user_id.clone()))
        .collect();
    let query = ids.join(" OR ");
    let profiles = auth0.find_info(&query).await?;

    let project_users_aggregated = project_users
        .into_iter()
        .map(|p| {
            let mut project_user = ProjectUserAggregated {
                user_id: p.user_id.clone(),
                project_id: p.project_id,
                name: "unknown".into(),
                email: "unknown".into(),
                role: p.role,
                created_at: p.created_at,
            };

            if let Some(profile) = profiles.iter().find(|a| a.user_id == p.user_id) {
                project_user.name.clone_from(&profile.name);
                project_user.email.clone_from(&profile.email)
            }

            project_user
        })
        .collect();

    Ok(project_users_aggregated)
}

pub async fn fetch_me_user(
    cache: Arc<dyn ProjectDrivenCache>,
    auth0: Arc<dyn Auth0Driven>,
    cmd: FetchMeUserCmd,
) -> Result<ProjectUserAggregated> {
    let user_id = assert_credential(&cmd.credential)?;
    assert_permission(cache.clone(), &cmd.credential, &cmd.project_id, None).await?;

    let Some(project_user) = cache
        .find_user_permission(&user_id, &cmd.project_id)
        .await?
    else {
        return Err(Error::CommandMalformed(
            "invalid project and user id".into(),
        ));
    };

    let profile = auth0.find_info(&format!("user_id:{user_id}")).await?;
    if profile.is_empty() {
        return Err(Error::Unexpected("Invalid user_id".into()));
    }
    let profile = profile.first().unwrap();

    let project_user_aggregated = ProjectUserAggregated {
        user_id: project_user.user_id.clone(),
        project_id: project_user.project_id,
        name: profile.name.clone(),
        email: profile.email.clone(),
        role: project_user.role,
        created_at: project_user.created_at,
    };

    Ok(project_user_aggregated)
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
    assert_permission(
        cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

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
    assert_permission(
        cache.clone(),
        &cmd.credential,
        &cmd.project_id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

    apply_user_invite(
        cache,
        email,
        event,
        &cmd.id,
        &cmd.project_id,
        &cmd.email,
        &cmd.role,
        cmd.ttl,
        false,
    )
    .await
}

/// Creates and sends a project invite on behalf of an operator, with no `Owner` check.
///
/// The sibling of `create_user_invite` for the backoffice, which holds no user credential — its
/// authority is possession of the producer credentials, not a role on the project. Naming that
/// bypass rather than letting a driver call into shared internals keeps the set of unauthenticated
/// entry points greppable.
///
/// With `dry_run` the project is still resolved, but no mail is sent and no event is dispatched.
#[allow(clippy::too_many_arguments)]
pub async fn create_user_invite_from_backoffice(
    cache: Arc<dyn ProjectDrivenCache>,
    email: Arc<dyn ProjectEmailDriven>,
    event: Arc<dyn EventDrivenBridge>,
    project_id: &str,
    invitee_email: &str,
    role: &ProjectUserRole,
    ttl: Duration,
    dry_run: bool,
) -> Result<()> {
    apply_user_invite(
        cache,
        email,
        event,
        &Uuid::new_v4().to_string(),
        project_id,
        invitee_email,
        role,
        ttl,
        dry_run,
    )
    .await
}

/// The body shared by `create_user_invite` and `create_user_invite_from_backoffice`.
///
/// Private on purpose: it performs no authorization, so every caller must be one of the two named
/// entry points above, which document the authority they rely on.
///
/// With `dry_run` this returns before the send. The email is the irreversible half — an invite
/// cannot be recalled — so it sits after the early return rather than before it, unlike the Stripe
/// call in `apply_ownership_transfer`.
#[allow(clippy::too_many_arguments)]
async fn apply_user_invite(
    cache: Arc<dyn ProjectDrivenCache>,
    email: Arc<dyn ProjectEmailDriven>,
    event: Arc<dyn EventDrivenBridge>,
    id: &str,
    project_id: &str,
    invitee_email: &str,
    role: &ProjectUserRole,
    ttl: Duration,
    dry_run: bool,
) -> Result<()> {
    let Some(project) = cache.find_by_id(project_id).await? else {
        return Err(Error::CommandMalformed("invalid project id".into()));
    };

    let code = Uuid::new_v4().to_string();

    let expires_in = Utc::now() + ttl;

    if dry_run {
        // The code is deliberately not logged. It is the bearer token that grants membership, and
        // this one is discarded unsent — but CLI output gets pasted around, and a code that looks
        // live invites confusion.
        info!(
            project = &project.name,
            email = invitee_email,
            role = %role,
            ?expires_in,
            "invite to send"
        );
        return Ok(());
    }

    email
        .send_invite(&project.name, invitee_email, &code, &expires_in)
        .await?;

    let evt = ProjectUserInviteCreated {
        id: id.to_string(),
        project_id: project.id,
        email: invitee_email.to_string(),
        role: role.to_string(),
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

    let profile = auth0.find_info(&format!("user_id:{user_id}")).await?;
    if profile.is_empty() {
        return Err(Error::Unexpected("Invalid user_id".into()));
    }
    let profile = profile.first().unwrap();

    if user_invite.email != profile.email {
        return Err(Error::CommandMalformed(
            "user email doesnt match with invite".into(),
        ));
    }

    let evt = ProjectUserInviteAccepted {
        id: user_invite.id,
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
        Some(ProjectUserRole::Owner),
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

pub async fn delete_user_invite(
    cache: Arc<dyn ProjectDrivenCache>,
    event: Arc<dyn EventDrivenBridge>,
    cmd: DeleteUserInviteCmd,
) -> Result<()> {
    let user_id = assert_credential(&cmd.credential)?;

    let Some(user_invite) = cache.find_user_invite_by_id(&cmd.id).await? else {
        return Err(Error::CommandMalformed("invalid invite id".into()));
    };

    assert_permission(
        cache.clone(),
        &cmd.credential,
        &user_invite.project_id,
        Some(ProjectUserRole::Owner),
    )
    .await?;

    if user_invite.status == ProjectUserInviteStatus::Accepted {
        return Err(Error::CommandMalformed("invite already accepted".into()));
    }

    let evt = ProjectUserInviteDeleted {
        id: cmd.id,
        project_id: user_invite.project_id,
        deleted_by: user_id,
        deleted_at: Utc::now(),
    };

    event.dispatch(evt.into()).await?;
    info!("project invite deleted");

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
pub struct FetchByIdCmd {
    pub credential: Credential,
    pub id: String,
}
impl FetchByIdCmd {
    pub fn new(credential: Credential, id: String) -> Self {
        Self { credential, id }
    }
}

#[derive(Debug, Clone)]
pub struct FetchByNamespaceCmd {
    pub credential: Credential,
    pub namespace: String,
}
impl FetchByNamespaceCmd {
    pub fn new(credential: Credential, namespace: String) -> Self {
        Self {
            credential,
            namespace,
        }
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
        let namespace = utils::get_random_name();

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
    pub project: String,
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct DeleteSecretCmd {
    pub credential: Credential,
    pub id: String,
}
impl DeleteSecretCmd {
    pub fn new(credential: Credential, id: String) -> Self {
        Self { credential, id }
    }
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
pub struct FetchMeUserCmd {
    pub credential: Credential,
    pub project_id: String,
}
impl FetchMeUserCmd {
    pub fn new(credential: Credential, project_id: String) -> Result<Self> {
        Ok(Self {
            credential,
            project_id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TransferOwnershipCmd {
    pub credential: Credential,
    pub project_id: String,
    pub new_owner: String,
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
    pub code: String,
}
impl AcceptUserInviteCmd {
    pub fn new(credential: Credential, code: String) -> Self {
        Self { credential, code }
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

#[derive(Debug, Clone)]
pub struct DeleteUserInviteCmd {
    pub credential: Credential,
    pub id: String,
}
impl DeleteUserInviteCmd {
    pub fn new(credential: Credential, id: String) -> Self {
        Self { credential, id }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::{
        auth::{Auth0Profile, MockAuth0Driven},
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
    impl Default for FetchByNamespaceCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                namespace: "sonic-vegas".into(),
            }
        }
    }
    impl Default for FetchByIdCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
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
                project: Default::default(),
                key: KEY.into(),
            }
        }
    }
    impl Default for DeleteSecretCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
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
    impl Default for DeleteUserInviteCmd {
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
    impl Default for FetchMeUserCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
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
    impl Default for TransferOwnershipCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                project_id: Uuid::new_v4().to_string(),
                new_owner: "member user id".into(),
            }
        }
    }
    impl Default for DeleteCmd {
        fn default() -> Self {
            Self {
                credential: Credential::Auth0("user id".into()),
                id: Uuid::new_v4().to_string(),
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
    async fn it_should_fetch_project_by_id() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let cmd = FetchByIdCmd::default();

        let result = fetch_by_id(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_by_id_when_invalid_permission() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let cmd = FetchByIdCmd::default();

        let result = fetch_by_id(Arc::new(cache), cmd).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_fetch_project_by_namespace() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_namespace()
            .return_once(|_| Ok(Some(Project::default())));

        let cmd = FetchByNamespaceCmd::default();

        let result = fetch_by_namespace(Arc::new(cache), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_by_namespace_when_invalid_permission() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_by_namespace()
            .return_once(|_| Ok(Some(Project::default())));
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(None));

        let cmd = FetchByNamespaceCmd::default();

        let result = fetch_by_namespace(Arc::new(cache), cmd).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_create_project() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_by_namespace().return_once(|_| Ok(None));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

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
    async fn it_should_fail_update_project_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });

        let event = MockEventDrivenBridge::new();

        let cmd = UpdateCmd::default();

        let result = update(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
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
    async fn it_should_fail_fetch_project_secrets_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });

        let cmd = FetchSecretCmd::default();

        let result = fetch_secret(Arc::new(cache), cmd).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
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
    async fn it_should_fail_create_project_secret_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });
        let event = MockEventDrivenBridge::new();

        let cmd = CreateSecretCmd::default();

        let result = create_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
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
    async fn it_should_delete_project_secret() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        cache
            .expect_find_secret_by_id()
            .return_once(|_| Ok(Some(ProjectSecret::default())));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = DeleteSecretCmd::default();

        let result = delete_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_delete_project_secret_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });
        cache
            .expect_find_secret_by_id()
            .return_once(|_| Ok(Some(ProjectSecret::default())));

        let event = MockEventDrivenBridge::new();

        let cmd = DeleteSecretCmd::default();

        let result = delete_secret(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
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
    async fn it_should_fail_fetch_project_user_invites_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });

        let cmd = FetchUserInviteCmd::default();

        let result = fetch_user_invite(Arc::new(cache), cmd).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
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
    async fn it_should_fail_create_project_user_invite_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });

        let email = MockProjectEmailDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = CreateUserInviteCmd::default();

        let result =
            create_user_invite(Arc::new(cache), Arc::new(email), Arc::new(event), cmd).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
    }

    #[tokio::test]
    async fn it_should_create_project_user_invite_from_backoffice() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        // No `expect_find_user_permission`: the backoffice entry point must not consult a role,
        // and mockall panics if it does.
        let mut email = MockProjectEmailDriven::new();
        email.expect_send_invite().return_once(|_, _, _, _| Ok(()));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let result = create_user_invite_from_backoffice(
            Arc::new(cache),
            Arc::new(email),
            Arc::new(event),
            "project id",
            "new@user.io",
            &ProjectUserRole::Member,
            Duration::from_secs(900),
            false,
        )
        .await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_not_send_mail_or_dispatch_on_a_dry_run_project_user_invite() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        // No expectations set: mockall panics on any call, so these assert the send and the
        // dispatch never happen. An invite email is not recallable, which is the whole point of
        // rehearsing one.
        let email = MockProjectEmailDriven::new();
        let event = MockEventDrivenBridge::new();

        let result = create_user_invite_from_backoffice(
            Arc::new(cache),
            Arc::new(email),
            Arc::new(event),
            "project id",
            "new@user.io",
            &ProjectUserRole::Member,
            Duration::from_secs(900),
            true,
        )
        .await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_create_project_user_invite_from_backoffice_when_project_doesnt_exist() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_by_id().return_once(|_| Ok(None));

        let email = MockProjectEmailDriven::new();
        let event = MockEventDrivenBridge::new();

        let result = create_user_invite_from_backoffice(
            Arc::new(cache),
            Arc::new(email),
            Arc::new(event),
            "project id",
            "new@user.io",
            &ProjectUserRole::Member,
            Duration::from_secs(900),
            false,
        )
        .await;

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
        auth0.expect_find_info().return_once(|_| {
            Ok(vec![Auth0Profile {
                email: invite_email,
                ..Default::default()
            }])
        });

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
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

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
    async fn it_should_fail_resend_project_user_invite_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });
        cache
            .expect_find_user_invite_by_id()
            .return_once(|_| Ok(Some(ProjectUserInvite::default())));

        let email = MockProjectEmailDriven::new();

        let cmd = ResendUserInviteCmd::default();

        let result = resend_user_invite(Arc::new(cache), Arc::new(email), cmd).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
    }

    #[tokio::test]
    async fn it_should_delete_project_user_invite() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_invite_by_id()
            .return_once(|_| Ok(Some(ProjectUserInvite::default())));
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = DeleteUserInviteCmd::default();

        let result = delete_user_invite(Arc::new(cache), Arc::new(event), cmd).await;

        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_delete_project_user_invite_when_invite_doesnt_exist() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_invite_by_id()
            .return_once(|_| Ok(None));

        let event = MockEventDrivenBridge::new();

        let cmd = DeleteUserInviteCmd::default();

        let result = delete_user_invite(Arc::new(cache), Arc::new(event), cmd).await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_delete_project_user_invite_when_invite_is_accepted() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_invite_by_id().return_once(|_| {
            Ok(Some(ProjectUserInvite {
                status: ProjectUserInviteStatus::Accepted,
                ..Default::default()
            }))
        });
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        let event = MockEventDrivenBridge::new();

        let cmd = DeleteUserInviteCmd::default();

        let result = delete_user_invite(Arc::new(cache), Arc::new(event), cmd).await;

        assert!(result.is_err());
    }
    #[tokio::test]
    async fn it_should_fail_delete_project_user_invite_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });
        cache
            .expect_find_user_invite_by_id()
            .return_once(|_| Ok(Some(ProjectUserInvite::default())));

        let event = MockEventDrivenBridge::new();

        let cmd = DeleteUserInviteCmd::default();

        let result = delete_user_invite(Arc::new(cache), Arc::new(event), cmd).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
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

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

        let cmd = FetchUserCmd::default();

        let result = fetch_user(Arc::new(cache), Arc::new(auth0), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_list_project_users() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_users()
            .return_once(|_, _, _| Ok(vec![ProjectUser::default()]));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

        let result = list_users(Arc::new(cache), Arc::new(auth0), "project id", &1, &12).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
    #[tokio::test]
    async fn it_should_not_query_auth0_when_the_page_has_no_users() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_users().return_once(|_, _, _| Ok(vec![]));

        // No expectation: an empty page must not reach Auth0 at all. An empty `q` is not a search
        // for nothing — it matches every user in the tenant.
        let auth0 = MockAuth0Driven::new();

        let result = list_users(Arc::new(cache), Arc::new(auth0), "project id", &9, &12).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
    #[tokio::test]
    async fn it_should_fail_fetch_project_users_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });

        let auth0 = MockAuth0Driven::new();

        let cmd = FetchUserCmd::default();

        let result = fetch_user(Arc::new(cache), Arc::new(auth0), cmd).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
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
    async fn it_should_transfer_project_ownership() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .times(2)
            .returning(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

        let mut stripe = MockStripeDriven::new();
        stripe
            .expect_update_customer()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().times(1).return_once(|_| Ok(()));

        let cmd = TransferOwnershipCmd::default();

        let result = transfer_ownership(
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
    async fn it_should_not_dispatch_transfer_ownership_on_dry_run() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .times(1)
            .returning(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

        // Neither mock has an expectation: both panic if called during a dry run.
        let stripe = MockStripeDriven::new();
        let event = MockEventDrivenBridge::new();

        let result = apply_ownership_transfer(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            Some(Arc::new(stripe)),
            "project id",
            "member user id",
            "backoffice",
            true,
        )
        .await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_transfer_ownership_without_touching_stripe_when_stripe_is_none() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .times(1)
            .returning(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

        // The event must still be dispatched — only the billing contact is left alone.
        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().times(1).return_once(|_| Ok(()));

        let result = apply_ownership_transfer(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            None,
            "project id",
            "member user id",
            "backoffice",
            false,
        )
        .await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_transfer_ownership_on_dry_run_when_new_owner_is_not_a_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .times(1)
            .returning(|_, _| Ok(None));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let auth0 = MockAuth0Driven::new();
        let stripe = MockStripeDriven::new();
        let event = MockEventDrivenBridge::new();

        let result = apply_ownership_transfer(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            Some(Arc::new(stripe)),
            "project id",
            "member user id",
            "backoffice",
            true,
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::CommandMalformed(_))));
    }
    #[tokio::test]
    async fn it_should_fail_transfer_ownership_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });

        let auth0 = MockAuth0Driven::new();
        let stripe = MockStripeDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = TransferOwnershipCmd::default();

        let result = transfer_ownership(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            Arc::new(stripe),
            cmd,
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
    }
    #[tokio::test]
    async fn it_should_fail_transfer_ownership_when_the_user_is_already_the_owner() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let auth0 = MockAuth0Driven::new();
        let stripe = MockStripeDriven::new();
        let event = MockEventDrivenBridge::new();

        // Project::default() is owned by "user id".
        let cmd = TransferOwnershipCmd {
            new_owner: "user id".into(),
            ..Default::default()
        };

        let result = transfer_ownership(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            Arc::new(stripe),
            cmd,
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::CommandMalformed(_))));
    }
    #[tokio::test]
    async fn it_should_fail_transfer_ownership_when_new_owner_is_not_a_member() {
        let mut cache = MockProjectDrivenCache::new();
        // First call authorizes the caller, second looks up the new owner.
        cache
            .expect_find_user_permission()
            .times(1)
            .returning(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_user_permission()
            .times(1)
            .returning(|_, _| Ok(None));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let auth0 = MockAuth0Driven::new();
        let stripe = MockStripeDriven::new();
        let event = MockEventDrivenBridge::new();

        let cmd = TransferOwnershipCmd::default();

        let result = transfer_ownership(
            Arc::new(cache),
            Arc::new(event),
            Arc::new(auth0),
            Arc::new(stripe),
            cmd,
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::CommandMalformed(_))));
    }
    #[tokio::test]
    async fn it_should_not_dispatch_transfer_ownership_when_stripe_fails() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .times(2)
            .returning(|_, _| Ok(Some(ProjectUser::default())));
        cache
            .expect_find_by_id()
            .return_once(|_| Ok(Some(Project::default())));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

        let mut stripe = MockStripeDriven::new();
        stripe
            .expect_update_customer()
            .returning(|_, _, _| Err(Error::Unexpected("stripe down".into())));

        // No dispatch expectation: the mock panics if the event is dispatched anyway.
        let event = MockEventDrivenBridge::new();

        let cmd = TransferOwnershipCmd::default();

        let result = transfer_ownership(
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
    async fn it_should_fail_delete_project_user_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });

        let event = MockEventDrivenBridge::new();

        let cmd = DeleteUserCmd::default();

        let result = delete_user(Arc::new(cache), Arc::new(event), cmd).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
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

    #[tokio::test]
    async fn it_should_delete_project() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        cache.expect_find_by_id().return_once(|_| {
            Ok(Some(Project {
                owner: "user id".into(),
                ..Default::default()
            }))
        });

        let mut event = MockEventDrivenBridge::new();
        event.expect_dispatch().return_once(|_| Ok(()));

        let cmd = DeleteCmd {
            credential: Credential::Auth0("user id".into()),
            ..Default::default()
        };

        let result = delete(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn it_should_fail_delete_project_when_invalid_permission_member() {
        let mut cache = MockProjectDrivenCache::new();
        cache.expect_find_user_permission().return_once(|_, _| {
            Ok(Some(ProjectUser {
                role: ProjectUserRole::Member,
                ..Default::default()
            }))
        });

        let event = MockEventDrivenBridge::new();

        let cmd = DeleteCmd::default();

        let result = delete(Arc::new(cache), Arc::new(event), cmd).await;

        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Unauthorized(_))));
    }
    #[tokio::test]
    async fn it_should_fail_delete_project_when_user_is_not_creator() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .return_once(|_, _| Ok(Some(ProjectUser::default())));

        cache.expect_find_by_id().return_once(|_| {
            Ok(Some(Project {
                owner: "user id".into(),
                ..Default::default()
            }))
        });

        let event = MockEventDrivenBridge::new();

        let cmd = DeleteCmd {
            credential: Credential::Auth0("user id member".into()),
            ..Default::default()
        };

        let result = delete(Arc::new(cache), Arc::new(event), cmd).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_should_fetch_me_project_user() {
        let mut cache = MockProjectDrivenCache::new();
        cache
            .expect_find_user_permission()
            .returning(|_, _| Ok(Some(ProjectUser::default())));

        let mut auth0 = MockAuth0Driven::new();
        auth0
            .expect_find_info()
            .return_once(|_| Ok(vec![Auth0Profile::default()]));

        let cmd = FetchMeUserCmd::default();

        let result = fetch_me_user(Arc::new(cache), Arc::new(auth0), cmd).await;
        assert!(result.is_ok());
    }
}
