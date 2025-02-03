use serde_json::to_string_pretty;
use slack_hook::{PayloadBuilder, Slack};
use std::sync::Arc;
use tracing::error;

use crate::domain::{
    auth::Auth0Driven, event::Event, notify::NotifyDriven, project::cache::ProjectDrivenCache,
    resource::cache::ResourceDrivenCache, Result,
};

static E2E_EMAIL: &str = "e2e@txpipe.io";

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
                    .find_info(&format!("user_id:{}", &payload.owner))
                    .await
                {
                    Ok(profile) => {
                        if profile.is_empty() {
                            Some(to_string_pretty(&payload).unwrap())
                        } else {
                            let profile = profile.first().unwrap();

                            let mut new_paload_as_value = serde_json::to_value(payload).unwrap();
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
                    Err(_) => Some(to_string_pretty(&payload).unwrap()),
                }
            }
            Event::ProjectDeleted(payload) => match project_cache.find_by_id(&payload.id).await {
                Ok(Some(project)) => {
                    match auth_driven
                        .find_info(&format!("user_id:{}", &project.owner))
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
                _ => None,
            },
            Event::ResourceCreated(payload) => match resource_cache.find_by_id(&payload.id).await {
                Ok(Some(resource)) => match project_cache.find_by_id(&resource.project_id).await {
                    Ok(Some(project)) => {
                        match auth_driven
                            .find_info(&format!("user_id:{}", &project.owner))
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
                    _ => None,
                },
                _ => None,
            },
            Event::ResourceUpdated(payload) => Some(to_string_pretty(&payload).unwrap()),
            Event::ResourceDeleted(payload) => match resource_cache.find_by_id(&payload.id).await {
                Ok(Some(resource)) => match project_cache.find_by_id(&resource.project_id).await {
                    Ok(Some(project)) => {
                        match auth_driven
                            .find_info(&format!("user_id:{}", &project.owner))
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
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };

        if let Some(data) = data {
            let message = format!("{}:\n```\n{}\n```", key, data);
            let payload = PayloadBuilder::new().text(message).build()?;
            if let Err(err) = self.client.send(&payload) {
                error!(err = err.to_string(), "Failed to notify to slack");
            }
        }
        Ok(())
    }
}
