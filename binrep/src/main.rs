#![allow(dead_code)]
#![allow(unused_variables)]
use anyhow::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use binrep_core::binrep::{parse_version_req, resolve_config};
use binrep_core::binrep::{Binrep, SyncStatus};
use binrep_core::exec::exec;
use binrep_core::metadata::Artifact;
use binrep_core::progress::InteractiveProgressReporter;
use binrep_core::semver::{Version, VersionReq};
use binrep_core::slack::{SlackConfig, WebhookConfig};
use binrep_core::slack_hook3::{AttachmentBuilder, PayloadBuilder};
use ring::signature::KeyPair;
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
enum UtilsOpt {
    /// Generate a base64 encoded ED25519 key pair.
    #[structopt(name = "gen-ed25519-keypair")]
    GenerateED25519KeyPar,
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
    #[structopt(name = "utils")]
    Utils(UtilsOpt),
}

#[derive(StructOpt)]
struct Opt {
    /// Configuration file, if not specified, default to ~/.binrep/config.sane and /etc/binrep/config.sane
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    config_file: Option<PathBuf>,
    #[structopt(subcommand)]
    command: Command,
}
#[tokio::main]
async fn main() {
    env_logger::init();
    let opt = Opt::from_args();
    if let Err(e) = _main(opt).await {
        eprintln!("{} - {:?}", e, e);
        std::process::exit(1);
    }
}

async fn _main(opt: Opt) -> Result<(), Error> {
    // If BINREP_CONFIG environment variable is provided, use it!
    let env_config = std::env::var("BINREP_CONFIG");
    let provided_config = match env_config {
        Ok(cfg) => Some(PathBuf::from(cfg)),
        Err(_) => opt.config_file.clone(),
    };

    let slack_configuration: SlackConfig = resolve_config(&provided_config)?;
    let mut binrep = Binrep::<InteractiveProgressReporter>::new(&provided_config)?;
    match opt.command {
        // LIST----------
        Command::List(opt) => match opt.artifact_name {
            None => print_list(binrep.list_artifacts().await?.artifacts),
            Some(artifact_name) => print_list(
                binrep
                    .list_artifact_versions(
                        &artifact_name,
                        &parse_optional_version_req(opt.version_req)?,
                    )
                    .await?,
            ),
        },
        Command::Push(opt) => {
            let artifact_name = &opt.artifact_name;
            let artifact_version = match opt.version.as_str() {
                "auto" => binrep
                    .last_version(artifact_name, &VersionReq::STAR)
                    .await
                    .or_else::<Error, _>(|e| {
                        // ignore errors and go on with default version
                        Ok(Some(Version::new(0, 0, 0)))
                    })?
                    .map(|mut v| {
                        v.patch += 1;
                        v
                    })
                    .unwrap_or(Version::new(0, 0, 1)),
                v => Version::parse(v)?,
            };
            let artifact_files = opt.files;
            let pushed = binrep
                .push(artifact_name, &artifact_version, &artifact_files)
                .await?;
            println!("Pushed {} {}", artifact_name, pushed);
            match send_slack_push_notif(&slack_configuration.into(), artifact_name, &pushed).await {
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
            let pulled = binrep
                .pull(artifact_name, &artifact_version, &destination_dir, true)
                .await?;
            println!("Pulled {} {}", artifact_name, pulled);
            exec(&pulled, &destination_dir, &opt.exec_command)?;
        }
        Command::Sync(opt) => {
            let artifact_name = &opt.artifact_name;
            let version_req = parse_version_req(&opt.version_req)?;
            let destination_dir = opt.destination_dir;
            let sync = binrep
                .sync(artifact_name, &version_req, &destination_dir)
                .await?;
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
            let artifact = binrep.artifact(artifact_name, &artifact_version).await?;
            println!("{} {}", artifact_name, artifact);
        }
        Command::Utils(opt) => match opt {
            UtilsOpt::GenerateED25519KeyPar => {
                let (priv_key, pub_key) =
                    generate_ed25519_key_pair().map_err(|ring_unspecified_error| {
                        anyhow::anyhow!({ ring_unspecified_error })
                    })?;
                println!(
                    "pkcs8: {}\npublic_key: {}",
                    data_encoding::BASE64.encode(&priv_key),
                    data_encoding::BASE64.encode(&pub_key)
                );
            }
        },
    }
    Ok(())
}

pub fn parse_optional_version_req(input: Option<String>) -> Result<VersionReq, Error> {
    Ok(match &input {
        None => VersionReq::STAR,
        Some(v) => parse_version_req(&v)?,
    })
}

fn print_list<T: Display, I: IntoIterator<Item = T>>(collection: I) {
    for item in collection {
        println!("{}", item);
    }
}

async fn send_slack_push_notif(
    slack: &WebhookConfig,
    artifact_name: &str,
    artifact: &Artifact,
) -> Result<bool, anyhow::Error> {
    slack
        .send(|| {
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
        .await
}

pub fn generate_ed25519_key_pair() -> Result<(Vec<u8>, Vec<u8>), ring::error::Unspecified> {
    let rng = ring::rand::SystemRandom::new();
    let pkcs8_bytes = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)?;
    let key_pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())?;
    let public_key = key_pair.public_key().as_ref().to_vec();
    Ok((pkcs8_bytes.as_ref().to_vec(), public_key))
}
