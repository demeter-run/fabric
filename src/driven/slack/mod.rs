use serde_json::to_string_pretty;
use slack_hook::{PayloadBuilder, Slack};
use std::sync::Arc;
use tracing::error;

use crate::domain::{
    auth::Auth0Driven,
    event::{Event, ProjectClaimed, ProvisionalProjectCreated},
    notify::NotifyDriven,
    project::{cache::ProjectDrivenCache, Project},
    resource::cache::ResourceDrivenCache,
    Result,
};

static E2E_EMAIL: &str = "e2e@txpipe.io";

/// Who a notification names for a project that has no owner yet.
fn provisional_user(agent_name: &str) -> String {
    format!("provisional, agent {agent_name}")
}

/// The notification of an event on a provisional project, which has no owner to look up.
fn provisional_message(payload: &impl serde::Serialize, project: &Project) -> String {
    let mut value = serde_json::to_value(payload).unwrap();
    value.as_object_mut().unwrap().insert(
        "user".to_string(),
        serde_json::json!(provisional_user(
            project.agent_name.as_deref().unwrap_or("unknown")
        )),
    );
    to_string_pretty(&value).unwrap()
}

/// The notification of a registration. Built field by field so the user code, which grants the
/// claim, and the hashes never reach the channel.
fn registration_message(payload: &ProvisionalProjectCreated) -> String {
    to_string_pretty(&serde_json::json!({
        "id": payload.id,
        "namespace": payload.namespace,
        "status": payload.status,
        "agent": {
            "name": payload.agent_name,
            "version": payload.agent_version,
            "url": payload.agent_url,
        },
        "expires_at": payload.expires_at,
        "user": provisional_user(&payload.agent_name),
    }))
    .unwrap()
}

/// The notification of a claim: who claimed the project, and which agent had registered it.
fn claim_message(payload: &ProjectClaimed, user: &str, agent_name: Option<&str>) -> String {
    let mut value = serde_json::to_value(payload).unwrap();
    value.as_object_mut().unwrap().insert(
        "user".to_string(),
        serde_json::json!(format!(
            "claimed by {user} (agent {})",
            agent_name.unwrap_or("unknown")
        )),
    );
    to_string_pretty(&value).unwrap()
}

pub struct SlackNotifyDrivenImpl {
    pub client: Slack,
}
impl SlackNotifyDrivenImpl {
    pub fn try_new(url: &str) -> Result<Self> {
        Ok(Self {
            client: Slack::new(url)?,
        })
    }
}

#[async_trait::async_trait]
impl NotifyDriven for SlackNotifyDrivenImpl {
    async fn notify(
        &self,
        evt: Event,
        auth_driven: Arc<dyn Auth0Driven>,
        resource_cache: Arc<dyn ResourceDrivenCache>,
        project_cache: Arc<dyn ProjectDrivenCache>,
    ) -> Result<()> {
        let key = &evt.key();
        let data: Option<String> = match evt {
            Event::ProjectCreated(payload) => {
                match auth_driven
                    .find_info(&format!("user_id:{}", payload.owner))
                    .await
                {
                    Ok(profile) => {
                        if profile.is_empty() {
                            Some(to_string_pretty(&payload).unwrap())
                        } else {
                            let profile = profile.first().unwrap();

                            if profile.email == E2E_EMAIL {
                                None
                            } else {
                                let mut new_paload_as_value =
                                    serde_json::to_value(payload).unwrap();
                                let new_payload = new_paload_as_value.as_object_mut().unwrap();
                                new_payload.insert(
                                    "user".to_string(),
                                    serde_json::json!({
                                        "name": profile.name,
                                        "email": profile.email
                                    }),
                                );
                                Some(to_string_pretty(&new_payload).unwrap())
                            }
                        }
                    }
                    Err(_) => Some(to_string_pretty(&payload).unwrap()),
                }
            }
            Event::ProvisionalProjectCreated(payload) => Some(registration_message(&payload)),
            Event::ProjectClaimed(payload) => {
                let agent_name = match project_cache.find_by_id(&payload.id).await {
                    Ok(Some(project)) => project.agent_name,
                    _ => None,
                };
                match auth_driven
                    .find_info(&format!("user_id:{}", payload.owner))
                    .await
                {
                    Ok(profile) => match profile.first() {
                        Some(profile) if profile.email == E2E_EMAIL => None,
                        Some(profile) => Some(claim_message(
                            &payload,
                            &format!("{} <{}>", profile.name, profile.email),
                            agent_name.as_deref(),
                        )),
                        None => Some(claim_message(
                            &payload,
                            &payload.owner,
                            agent_name.as_deref(),
                        )),
                    },
                    Err(_) => Some(claim_message(
                        &payload,
                        &payload.owner,
                        agent_name.as_deref(),
                    )),
                }
            }
            Event::ProjectDeleted(payload) => match project_cache.find_by_id(&payload.id).await {
                Ok(Some(project)) => match &project.owner {
                    None => Some(provisional_message(&payload, &project)),
                    Some(owner) => match auth_driven.find_info(&format!("user_id:{owner}")).await {
                        Ok(profile) => {
                            if profile.is_empty() {
                                Some(to_string_pretty(&payload).unwrap())
                            } else {
                                let profile = profile.first().unwrap();
                                if profile.email == E2E_EMAIL {
                                    None
                                } else {
                                    let mut new_paload_as_value =
                                        serde_json::to_value(payload).unwrap();
                                    let new_payload = new_paload_as_value.as_object_mut().unwrap();
                                    new_payload.insert(
                                        "user".to_string(),
                                        serde_json::json!({
                                            "name": profile.name,
                                            "email": profile.email
                                        }),
                                    );
                                    Some(to_string_pretty(&new_payload).unwrap())
                                }
                            }
                        }
                        Err(_) => Some(to_string_pretty(&payload).unwrap()),
                    },
                },
                _ => None,
            },
            Event::ResourceCreated(payload) => match resource_cache.find_by_id(&payload.id).await {
                Ok(Some(resource)) => match project_cache.find_by_id(&resource.project_id).await {
                    Ok(Some(project)) => match &project.owner {
                        None => Some(provisional_message(&payload, &project)),
                        Some(owner) => {
                            match auth_driven.find_info(&format!("user_id:{owner}")).await {
                                Ok(profile) => {
                                    if profile.is_empty() {
                                        Some(to_string_pretty(&payload).unwrap())
                                    } else {
                                        let profile = profile.first().unwrap();
                                        if profile.email == E2E_EMAIL {
                                            None
                                        } else {
                                            let mut new_paload_as_value =
                                                serde_json::to_value(payload).unwrap();
                                            let new_payload =
                                                new_paload_as_value.as_object_mut().unwrap();
                                            new_payload.insert(
                                                "user".to_string(),
                                                serde_json::json!({
                                                    "name": profile.name,
                                                    "email": profile.email
                                                }),
                                            );
                                            Some(to_string_pretty(&new_payload).unwrap())
                                        }
                                    }
                                }
                                Err(_) => Some(to_string_pretty(&payload).unwrap()),
                            }
                        }
                    },
                    _ => None,
                },
                _ => None,
            },
            Event::ResourceUpdated(payload) => Some(to_string_pretty(&payload).unwrap()),
            Event::ResourceDeleted(payload) => match resource_cache.find_by_id(&payload.id).await {
                Ok(Some(resource)) => match project_cache.find_by_id(&resource.project_id).await {
                    Ok(Some(project)) => match &project.owner {
                        None => Some(provisional_message(&payload, &project)),
                        Some(owner) => {
                            match auth_driven.find_info(&format!("user_id:{owner}")).await {
                                Ok(profile) => {
                                    if profile.is_empty() {
                                        Some(to_string_pretty(&payload).unwrap())
                                    } else {
                                        let profile = profile.first().unwrap();
                                        if profile.email == E2E_EMAIL {
                                            None
                                        } else {
                                            let mut new_paload_as_value =
                                                serde_json::to_value(payload).unwrap();
                                            let new_payload =
                                                new_paload_as_value.as_object_mut().unwrap();
                                            new_payload.insert(
                                                "user".to_string(),
                                                serde_json::json!({
                                                    "name": profile.name,
                                                    "email": profile.email
                                                }),
                                            );
                                            Some(to_string_pretty(&new_payload).unwrap())
                                        }
                                    }
                                }
                                Err(_) => Some(to_string_pretty(&payload).unwrap()),
                            }
                        }
                    },
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };

        if let Some(data) = data {
            let message = format!("{key}:\n```\n{data}\n```");
            let payload = PayloadBuilder::new().text(message).build()?;
            if let Err(err) = self.client.send(&payload) {
                error!(err = err.to_string(), "Failed to notify to slack");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_keep_secrets_out_of_the_registration_message() {
        let payload = ProvisionalProjectCreated::default();

        let message = registration_message(&payload);

        assert!(message.contains("provisional, agent claude-code"));
        assert!(message.contains(&payload.namespace));
        assert!(!message.contains(&payload.user_code));
        assert!(!message.contains(&payload.claim_token_hash));
        assert!(!message.contains(&payload.source_ip_hash));
    }

    #[test]
    fn it_should_name_the_claimer_and_the_agent_in_the_claim_message() {
        let payload = ProjectClaimed::default();

        let message = claim_message(&payload, "Jane <jane@example.com>", Some("claude-code"));
        assert!(message.contains("claimed by Jane <jane@example.com> (agent claude-code)"));
        assert!(message.contains(&payload.id));

        let message = claim_message(&payload, "user id", None);
        assert!(message.contains("claimed by user id (agent unknown)"));
    }

    #[test]
    fn it_should_name_the_agent_for_a_project_with_no_owner() {
        let project = Project {
            owner: None,
            agent_name: Some("claude-code".into()),
            ..Default::default()
        };

        let message = provisional_message(&serde_json::json!({"id": "x"}), &project);
        assert!(message.contains("provisional, agent claude-code"));
    }
}
