use serde_json::to_string_pretty;
use slack_hook::{PayloadBuilder, Slack};
use std::sync::Arc;
use tracing::error;

use crate::domain::{auth::Auth0Driven, event::Event, notify::NotifyDriven, Result};

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
    async fn notify(&self, evt: Event, auth_driven: Arc<dyn Auth0Driven>) -> Result<()> {
        let key = &evt.key();
        let data: Option<String> = match evt {
            Event::ProjectCreated(payload) => match auth_driven.find_info(&payload.owner).await {
                Ok((name, email)) => {
                    let mut new_paload_as_value = serde_json::to_value(payload).unwrap();
                    let new_payload = new_paload_as_value.as_object_mut().unwrap();
                    new_payload.insert(
                        "user".to_string(),
                        serde_json::json!({
                            "name": name,
                            "email": email
                        }),
                    );
                    Some(to_string_pretty(&new_payload).unwrap())
                }
                Err(_) => Some(to_string_pretty(&payload).unwrap()),
            },
            Event::ProjectDeleted(payload) => Some(to_string_pretty(&payload).unwrap()),
            Event::ResourceCreated(payload) => Some(to_string_pretty(&payload).unwrap()),
            Event::ResourceUpdated(payload) => Some(to_string_pretty(&payload).unwrap()),
            Event::ResourceDeleted(payload) => Some(to_string_pretty(&payload).unwrap()),
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
