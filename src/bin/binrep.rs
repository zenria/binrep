#![allow(dead_code)]
#![allow(unused_variables)]
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use binrep::binrep::parse_version_req;
use binrep::binrep::{Binrep, SyncStatus};
use binrep::exec::exec;
use binrep::metadata::Artifact;
use binrep::slack::{SlackConfig, WebhookConfig};
use semver::{Version, VersionReq};
use slack_hook::{AttachmentBuilder, PayloadBuilder};
use std::fmt::Display;

#[derive(StructOpt)]
struct PullOpt {
    /// Command to execute after the artifact has been successfully pulled
    #[structopt(short = "e", long = "exec")]
    exec_command: Option<String>,
    artifact_name: String,
    version: String,
    #[structopt(parse(from_os_str))]
    destination_dir: PathBuf,
}

#[derive(StructOpt)]
struct SyncOpt {
    /// Command to execute if the artifact has been updated (a new version has been pulled)
    #[structopt(short = "e", long = "exec")]
    exec_command: Option<String>,
    artifact_name: String,
    /// Version requirement (eg: *, 1.x, ^1.0.0, ~1, latest)
    version_req: String,
    #[structopt(parse(from_os_str))]
    destination_dir: PathBuf,
}

#[derive(StructOpt)]
struct PushOpt {
    artifact_name: String,
    version: String,
    #[structopt(parse(from_os_str))]
    files: Vec<PathBuf>,
}
#[derive(StructOpt)]
struct InspectOpt {
    artifact_name: String,
    version: String,
}

#[derive(StructOpt)]
struct ListOpt {
    /// artifact name
    artifact_name: Option<String>,
    /// artifact version requirement
    version_req: Option<String>,
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "push")]
    Push(PushOpt),
    #[structopt(name = "pull")]
    Pull(PullOpt),
    #[structopt(name = "ls")]
    List(ListOpt),
    #[structopt(name = "sync")]
    Sync(SyncOpt),
    #[structopt(name = "inspect")]
    Inspect(InspectOpt),
}

#[derive(StructOpt)]
struct Opt {
    /// Configuration file, if not specified, default to ~/.binrep/config.sane and /etc/binrep/config.sane
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config_file: Option<PathBuf>,
    #[structopt(subcommand)]
    command: Command,
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
    let slack_configuration: SlackConfig = Binrep::resolve_config(&opt.config_file)?;
    let binrep = Binrep::new(&opt.config_file)?;
    match opt.command {
        // LIST----------
        Command::List(opt) => match opt.artifact_name {
            None => print_list(binrep.list_artifacts()?.artifacts),
            Some(artifact_name) => print_list(binrep.list_artifact_versions(
                &artifact_name,
                &parse_optional_version_req(opt.version_req)?,
            )?),
        },
        Command::Push(opt) => {
            let artifact_name = &opt.artifact_name;
            let artifact_version = match opt.version.as_str() {
                "auto" => binrep
                    .last_version(artifact_name, &VersionReq::any())
                    .or_else::<Error, _>(|e| {
                        // ignore errors and go on with default version
                        Ok(Some((0, 0, 0).into()))
                    })?
                    .map(|mut v| {
                        v.increment_patch();
                        v
                    })
                    .unwrap_or((0, 0, 1).into()),
                v => Version::parse(v)?,
            };
            let artifact_files = opt.files;
            let pushed = binrep.push(artifact_name, &artifact_version, &artifact_files)?;
            println!("Pushed {} {}", artifact_name, pushed);
            match send_slack_push_notif(&slack_configuration.into(), artifact_name, &pushed) {
                Ok(sent) => {
                    if sent {
                        println!("Slack notification sent.");
                    }
                }
                Err(e) => eprintln!("Cannot send slack notification: {}", e),
            }
        }
        Command::Pull(opt) => {
            let artifact_name = &opt.artifact_name;
            let artifact_version = Version::parse(&opt.version)?;
            let destination_dir = opt.destination_dir;
            let pulled = binrep.pull(artifact_name, &artifact_version, &destination_dir, true)?;
            println!("Pulled {} {}", artifact_name, pulled);
            exec(&pulled, &destination_dir, &opt.exec_command)?;
        }
        Command::Sync(opt) => {
            let artifact_name = &opt.artifact_name;
            let version_req = parse_version_req(&opt.version_req)?;
            let destination_dir = opt.destination_dir;
            let sync = binrep.sync(artifact_name, &version_req, &destination_dir)?;
            let print_output = opt.exec_command.is_none();
            match sync.status {
                SyncStatus::UpToDate => {
                    if print_output {
                        println!("Nothing pulled, files are in sync");
                    }
                }
                SyncStatus::Updated => {
                    if print_output {
                        println!("Updated {} to {}", artifact_name, sync.artifact);
                    }
                    exec(&sync.artifact, &destination_dir, &opt.exec_command)?;
                }
            }
        }
        Command::Inspect(opt) => {
            let artifact_name = &opt.artifact_name;
            let artifact_version = Version::parse(&opt.version)?;
            let artifact = binrep.artifact(artifact_name, &artifact_version)?;
            println!("{} {}", artifact_name, artifact);
        }
    }
    Ok(())
}

pub fn parse_optional_version_req(input: Option<String>) -> Result<VersionReq, Error> {
    Ok(match &input {
        None => VersionReq::any(),
        Some(v) => parse_version_req(&v)?,
    })
}

fn print_list<T: Display, I: IntoIterator<Item = T>>(collection: I) {
    for item in collection {
        println!("{}", item);
    }
}

fn send_slack_push_notif(
    slack: &WebhookConfig,
    artifact_name: &str,
    artifact: &Artifact,
) -> Result<bool, slack_hook::Error> {
    slack.send(|| {
        let files: String = artifact
            .files
            .iter()
            .map(|file| format!("\n- `{}`", file.name))
            .collect();
        let files_text = format!(
            "{} file{} uploaded: {}",
            artifact.files.len(),
            if artifact.files.len() > 1 { "s" } else { "" },
            files
        );
        Ok(PayloadBuilder::new()
            .text(format!(
                "Pushed version *{}* of *{}* to artifact repository.",
                artifact.version, artifact_name
            ))
            .attachments(vec![AttachmentBuilder::new(files_text.clone())
                .text(files_text)
                .color("good")
                .build()?]))
    })
}
