//! Helper module for slack notifications
use serde::Deserialize;
use serde::Serialize;
use slack_hook::{PayloadBuilder, Result, Slack};

/// A config where any value is optional ;)
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct WebhookConfig {
    webhook_url: Option<String>,
    channel: Option<String>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        WebhookConfig {
            webhook_url: None,
            channel: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SlackConfig {
    slack: Option<WebhookConfig>,
}

impl WebhookConfig {
    pub fn override_with(&self, config: WebhookConfig) -> Self {
        Self {
            webhook_url: config.webhook_url.or(self.webhook_url.clone()),
            channel: config.channel.or(self.channel.clone()),
        }
    }

    pub fn send<F: Fn() -> Result<PayloadBuilder>>(&self, payload_builder: F) -> Result<bool> {
        if let Some(webhook_url) = &self.webhook_url {
            // config has at least a url somewhere!

            // build payload with supplied builder
            let payload_builder = payload_builder()?;
            let payload_builder = if let Some(channel) = &self.channel {
                // override channel
                payload_builder.channel(channel)
            } else {
                payload_builder
            };

            Slack::new(webhook_url.as_str())?.send(&payload_builder.build()?)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl From<SlackConfig> for WebhookConfig {
    fn from(s: SlackConfig) -> Self {
        match s.slack {
            None => WebhookConfig::default(),
            Some(inner) => inner,
        }
    }
}
