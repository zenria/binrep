//! Helper module for slack notifications
use serde::Deserialize;
use serde::Serialize;
use slack_hook::{PayloadBuilder, Result, Slack};

/// A config where any value is optional ;)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebhookConfig {
    webhook_url: Option<String>,
    channel: Option<String>,
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
}

impl SlackConfig {
    pub fn override_with(&self, config: SlackConfig) -> Self {
        if let Some(webhook_config) = config.slack {
            Self {
                slack: Some(if let Some(to_override) = self.slack.as_ref() {
                    // we have something to override
                    to_override.override_with(webhook_config)
                } else {
                    // just take the new config
                    webhook_config
                }),
            }
        } else {
            self.clone()
        }
    }
    pub fn send<F: Fn() -> Result<PayloadBuilder>>(&self, payload_builder: F) -> Result<bool> {
        if let Some(webhook_config) = &self.slack {
            if let Some(webhook_url) = &webhook_config.webhook_url {
                // config has at least a url somewhere!

                // build payload with supplied builder
                let payload_builder = payload_builder()?;
                let payload_builder = if let Some(channel) = &webhook_config.channel {
                    // override channel
                    payload_builder.channel(channel)
                } else {
                    payload_builder
                };

                Slack::new(webhook_url.as_str())?.send(&payload_builder.build()?)?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_override() {
        let empty: SlackConfig = sane::from_str("").unwrap();
        let empty_values: SlackConfig = sane::from_str(
            r#"
            slack = {}
        "#,
        )
        .unwrap();
        let barurl: SlackConfig = sane::from_str(
            r#"
            slack = {
                webhook_url="bar"
            }
        "#,
        )
        .unwrap();

        let foourl: SlackConfig = sane::from_str(
            r#"
            slack = {webhook_url="foo"}
        "#,
        )
        .unwrap();

        assert_eq!(
            "bar",
            empty
                .override_with(barurl.clone())
                .slack
                .unwrap()
                .webhook_url
                .unwrap()
                .as_str()
        );
        assert_eq!(
            "bar",
            empty_values
                .override_with(barurl.clone())
                .slack
                .unwrap()
                .webhook_url
                .unwrap()
                .as_str()
        );
        assert_eq!(
            "bar",
            foourl
                .override_with(barurl.clone())
                .slack
                .unwrap()
                .webhook_url
                .unwrap()
                .as_str()
        );

        assert_eq!(
            "bar",
            barurl
                .override_with(empty.clone())
                .slack
                .unwrap()
                .webhook_url
                .unwrap()
                .as_str()
        );
        assert_eq!(
            "bar",
            barurl
                .override_with(empty_values.clone())
                .slack
                .unwrap()
                .webhook_url
                .unwrap()
                .as_str()
        );
    }
}
