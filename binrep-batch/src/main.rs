//! Read a config file containing a list of binrep operation to perform.

#![allow(dead_code)]
#![allow(unused_variables)]
use anyhow::{Context, Error};
use std::path::PathBuf;
use structopt::StructOpt;

use binrep_core::binrep::Binrep;
use binrep_core::config_resolver::resolve_config;
use binrep_core::{binrep, file_utils};
use glob::glob;
use serde::Deserialize;
use serde::Serialize;

use binrep_core::extended_exec::{Line, Type};
use binrep_core::progress::InteractiveProgressReporter;
use binrep_core::slack::{SlackConfig, WebhookConfig};
use log::debug;
use slack_hook::PayloadBuilder;

#[derive(StructOpt)]
struct Opt {
    /// Configuration file, if not specified, default to ~/.binrep/config.sane and /etc/binrep/config.sane
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config_file: Option<PathBuf>,
    /// batch configuration file, if not provided default to  ~/.binrep/batch.sane
    /// and /etc/binrep/batch.sane
    batch_configuration_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct SyncOperation {
    #[serde(rename = "name")]
    pub artifact_name: String,
    #[serde(rename = "version")]
    pub version_req: String,
    #[serde(rename = "destination")]
    pub destination_dir: String,
    pub exec: Option<String>,
    pub slack: Option<SlackNotifier>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize, Clone)]
pub struct SlackNotifier {
    pub enabled: bool,
    #[serde(flatten)]
    pub webhook_config: WebhookConfig,
}

impl SlackNotifier {
    fn merge_with_default(self, default: &SlackNotifier) -> Self {
        let webhook_config = default.webhook_config.override_with(self.webhook_config);
        let enabled = self.enabled;
        Self {
            webhook_config,
            enabled,
        }
    }

    pub fn send<F: Fn() -> slack_hook::Result<PayloadBuilder>>(
        &self,
        payload_builder: F,
    ) -> slack_hook::Result<bool> {
        if self.enabled {
            self.webhook_config.send(payload_builder)
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct BatchConfig {
    /// eg. includes=/etc/binrep/batch.d/*.sane
    includes: Option<String>,
    #[serde(rename = "sync")]
    sync_operations: Vec<SyncOperation>,
    slack: Option<SlackNotifier>,
}

fn main() {
    env_logger::init();
    let opt = Opt::from_args();
    if let Err(e) = _main(opt) {
        eprintln!("{} - {:?}", e, e);
        std::process::exit(1);
    }
}
fn _main(opt: Opt) -> Result<(), Error> {
    // ---- parse Batch config
    let batch_config: BatchConfig = resolve_config(&opt.batch_configuration_file, "batch.sane").context("Unable to read batch.sane configuration file.")?;

    // ---- parse slack section of binrep config
    // get root slack config
    let slack_configuration: SlackConfig = binrep::resolve_config(&opt.config_file)?;
    let webhook_config: WebhookConfig = slack_configuration.into();
    // override root config with batch config
    let webhook_config = webhook_config.override_with(
        batch_config
            .slack
            .as_ref()
            .map(|n| n.webhook_config.clone())
            .unwrap_or(WebhookConfig::default()),
    );
    let default_slack_notifier = SlackNotifier {
        webhook_config,
        enabled: batch_config.slack.map(|s| s.enabled).unwrap_or(false),
    };

    // ----- setup binrep
    let mut binrep = Binrep::<InteractiveProgressReporter>::new(&opt.config_file)?;

    // ----- SYNC!!
    let operations: Vec<SyncOperation> = batch_config
        .sync_operations
        .into_iter()
        .chain(get_operation_from_includes(batch_config.includes))
        .collect();

    batch::sync(&mut binrep, operations, default_slack_notifier)?;
    Ok(())
}

fn get_operation_from_includes(includes: Option<String>) -> Vec<SyncOperation> {
    includes
        .map(|includes_path| glob(&includes_path).expect("Failed to read glob pattern"))
        .into_iter()
        .flatten()
        .map(|path| path.unwrap())
        .map(|path| {
            debug!("Reading included config file {:?}", path);
            file_utils::read_sane_from_file::<_, BatchConfig>(&path)
                .with_context(||format!("Unable to read {}", path.to_string_lossy()))
                .unwrap()
                .sync_operations
        })
        .flatten()
        .collect()
}

mod batch {
    use crate::{execution_commands_to_text, SlackNotifier};
    use anyhow::Error;
    use binrep_core::binrep::{parse_version_req, Binrep, SyncStatus};
    use binrep_core::exec::{exec, ExecutionError};
    use binrep_core::extended_exec::Line;
    use binrep_core::metadata::Artifact;
    use binrep_core::progress::ProgressReporter;
    use semver::VersionReq;
    use slack_hook::{AttachmentBuilder, PayloadBuilder};
    use std::convert::{TryFrom, TryInto};
    use std::path::PathBuf;

    struct SyncOperation {
        artifact_name: String,
        version_req: VersionReq,
        destination_dir: PathBuf,
        command: Option<String>,
        slack: Option<SlackNotifier>,
    }

    impl TryFrom<super::SyncOperation> for SyncOperation {
        type Error = Error;

        fn try_from(value: super::SyncOperation) -> Result<Self, Self::Error> {
            Ok(SyncOperation {
                artifact_name: value.artifact_name,
                version_req: parse_version_req(&value.version_req)?,
                destination_dir: PathBuf::from(value.destination_dir),
                command: value.exec,
                slack: value.slack,
            })
        }
    }

    pub fn sync<T>(
        binrep: &mut Binrep<T>,
        operations: Vec<super::SyncOperation>,
        default_slack_notifier: SlackNotifier,
    ) -> Result<(), Error>
    where
        T: ProgressReporter + 'static,
        T::Output: Send + Sync + 'static,
    {
        // validate config
        let operations: Vec<SyncOperation> = operations.into_iter().try_fold(
            Vec::new(),
            |mut acc, op| -> Result<Vec<SyncOperation>, Error> {
                acc.push(op.try_into()?);
                Ok(acc)
            },
        )?;
        for operation in operations {
            println!(
                "Syncing {} to {}",
                operation.artifact_name,
                operation.destination_dir.to_string_lossy()
            );
            let result = binrep.sync(
                &operation.artifact_name,
                &operation.version_req,
                &operation.destination_dir,
            )?;
            let slack_notifier = if let Some(op_slack_notifier) = &operation.slack {
                op_slack_notifier
                    .clone()
                    .merge_with_default(&default_slack_notifier)
            } else {
                default_slack_notifier.clone()
            };
            match &result.status {
                SyncStatus::Updated => {
                    println!("Updated: {}", result.artifact);
                    match handle_exec_result(
                        exec(
                            &result.artifact,
                            &operation.destination_dir,
                            &operation.command,
                        ),
                        &slack_notifier,
                        &operation.artifact_name,
                        &result.artifact,
                    ) {
                        Ok(sent) => {
                            if sent {
                                println!("Slack notification sent!");
                            }
                        }
                        Err(e) => {
                            eprintln!("Cannot send slack notification: {}", e);
                        }
                    }
                }
                SyncStatus::UpToDate => {
                    println!("Already the latest version {}", result.artifact.version);
                }
            }
        }
        Ok(())
    }

    fn handle_exec_result(
        exec_result: Result<Option<Vec<Line>>, Error>,
        slack_notifier: &SlackNotifier,
        artifact_name: &str,
        artifact: &Artifact,
    ) -> Result<bool, slack_hook::Error> {
        let hostname = hostname::get()
            .ok()
            .map(|hostname| hostname.to_string_lossy().into_owned())
            .unwrap_or("#unknown".into());
        match exec_result {
            Ok(output_lines) => slack_notifier.send(|| {
                let updated_text = format!(
                    "Updated *{}* to version *{}* on *{}*.",
                    artifact_name, artifact.version, hostname
                );
                Ok(PayloadBuilder::new().text(updated_text).attachments(
                    output_lines
                        .iter()
                        .filter(|lines| lines.len() > 0)
                        .flat_map(|lines| {
                            let command_text = execution_commands_to_text(lines);
                            AttachmentBuilder::new(command_text.clone())
                                .text(command_text)
                                .color("good")
                                .build()
                                .into_iter()
                        })
                        .collect(),
                ))
            }),
            Err(e) => {
                eprintln!("Execution error: {}", e);
                slack_notifier.send(|| {
                    let updated_text = format!(
                        "Something went wrong updating *{}* to version *{}* on *{}*.\n```\n{}```",
                        artifact_name, artifact.version, hostname, e
                    );
                    let lines = e.downcast_ref::<ExecutionError>().map(|e| &e.output_lines);
                    Ok(PayloadBuilder::new().text(updated_text).attachments(
                        lines
                            .iter()
                            .filter(|lines| lines.len() > 0)
                            .flat_map(|lines| {
                                let command_text = execution_commands_to_text(lines);
                                AttachmentBuilder::new(command_text.clone())
                                    .text(command_text)
                                    .color("danger")
                                    .build()
                                    .into_iter()
                            })
                            .collect(),
                    ))
                })
            }
        }
    }
}

fn type_to_string(line_type: Type) -> &'static str {
    match line_type {
        Type::Out => "o>",
        Type::Err => "e>",
        Type::Cmd => "cmd>",
    }
}

fn execution_commands_to_text(lines: &[Line]) -> String {
    let output: String = lines
        .iter()
        .map(|line| {
            format!(
                "{} {}\n",
                type_to_string(line.line_type),
                String::from_utf8_lossy(&line.line)
            )
        })
        .collect();
    format!("Command execution summary:\n```\n{}```", output)
}

#[cfg(test)]
mod test {
    use crate::BatchConfig;
    use crate::{get_operation_from_includes, SyncOperation};
    use binrep_core::file_utils;

    #[test]
    fn test_config() {
        // sane syntax
        let c = r#"sync = [
            {
                name="binrep",
                version="*",
                destination="/srv/dist/binrep/bin"
            },
            {
                name="binrep-bootstrap",
                version="2",
                destination="/srv/www/binrep-bootstrap",
                exec="echo hello"
            },
        ]"#;
        sane::from_str::<BatchConfig>(c).unwrap();
        // our parser also accepts toml syntax
        let c = r#"[[sync]]
            name="binrep"
            version="*"
            destination="/srv/dist/binrep/bin"
            [[sync]]
            name="binrep-bootstrap"
            version="2"
            destination="/srv/www/binrep-bootstrap"
            exec="echo hello"
            slack={ enabled=true }
        "#;
        sane::from_str::<BatchConfig>(c).unwrap();
        // test empty config

        sane::from_str::<BatchConfig>("sync=[]").unwrap();

        sane::from_str::<BatchConfig>("includes=\"/etc/batch.d/*.sync\"\nsync=[]").unwrap();

        assert_eq!(
            Vec::<SyncOperation>::new(),
            get_operation_from_includes(None)
        );
        assert_eq!(
            Vec::<SyncOperation>::new(),
            get_operation_from_includes(Some("src/non-exising/*.sane".into()))
        );
        let temp_dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Vec::<SyncOperation>::new(),
            get_operation_from_includes(Some(format!(
                "{}/*.sane",
                temp_dir.path().to_string_lossy()
            )))
        );

        let file1 = file_utils::path_concat2(&temp_dir, "coucou.sane");
        let operations1 = BatchConfig {
            sync_operations: vec![SyncOperation {
                artifact_name: "coucou".to_string(),
                version_req: "latest".to_string(),
                destination_dir: "/tmp/abcde".to_string(),
                exec: None,
                slack: None,
            }],
            includes: None,
            slack: None,
        };
        file_utils::write_sane_to_file(&file1, &operations1).unwrap();

        let file2 = file_utils::path_concat2(&temp_dir, "coucou2.sane");
        let operations2 = BatchConfig {
            sync_operations: vec![
                SyncOperation {
                    artifact_name: "coucou1".to_string(),
                    version_req: "1.3.0".to_string(),
                    destination_dir: "/tmp/abcdef".to_string(),
                    exec: None,
                    slack: None,
                },
                SyncOperation {
                    artifact_name: "coucou2".to_string(),
                    version_req: "1.0.3".to_string(),
                    destination_dir: "/tmp/abcdsdsdef".to_string(),
                    exec: None,
                    slack: None,
                },
            ],
            includes: None,
            slack: None,
        };
        file_utils::write_sane_to_file(&file2, &operations2).unwrap();

        assert_eq!(
            operations1
                .sync_operations
                .into_iter()
                .chain(operations2.sync_operations.into_iter())
                .collect::<Vec<_>>(),
            get_operation_from_includes(Some(format!(
                "{}/*.sane",
                temp_dir.path().to_string_lossy()
            )))
        );
    }
}
