#![allow(dead_code)]
#![allow(unused_variables)]
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use binrep::binrep::{Binrep, SyncStatus};
use semver::{Version, VersionReq};
use std::fmt::Display;

#[derive(StructOpt)]
struct PullOpt {
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

/// Usage
///
///
/// binrep push artifact_name artifact_version files...
/// binrep pull artifact_name latest output_dir
/// binrep pull artifact_name artifact_version output_dir
/// binrep ls
/// binrep ls artifact_name
/// binrep ls artifact_name/latest
/// binrep ls artifact_name/version
///
/// config searched in
///   ~/.binrep/config.sane
///   /etc/binrep/config.sane
/// or manually specified with -c eg
///
fn main() {
    env_logger::init();
    let opt = Opt::from_args();
    if let Err(e) = _main(opt) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn _main(opt: Opt) -> Result<(), Error> {
    let binrep = Binrep::new(opt.config_file)?;
    match opt.command {
        // LIST----------
        Command::List(opt) => match opt.artifact_name {
            None => print_list(binrep.list_artifacts()?.artifacts),
            Some(artifact_name) => print_list(
                binrep
                    .list_artifact_versions(&artifact_name, &parse_version_req(opt.version_req)?)?,
            ),
        },
        Command::Push(opt) => {
            let artifact_name = &opt.artifact_name;
            let artifact_version = Version::parse(&opt.version)?;
            let artifact_files = opt.files;
            let pushed = binrep.push(artifact_name, &artifact_version, &artifact_files)?;
            println!("Pushed {} {}", artifact_name, pushed);
        }
        Command::Pull(opt) => {
            let artifact_name = &opt.artifact_name;
            let artifact_version = Version::parse(&opt.version)?;
            let destination_dir = opt.destination_dir;
            let pushed = binrep.pull(artifact_name, &artifact_version, &destination_dir, true)?;
            println!("Pulled {} {}", artifact_name, pushed);
        }
        Command::Sync(opt) => {
            let artifact_name = &opt.artifact_name;
            let version_req = parse_version_req(Some(opt.version_req))?;
            let destination_dir = opt.destination_dir;
            let sync = binrep.sync(artifact_name, &version_req, destination_dir)?;
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
                    match opt.exec_command {
                        None => (),
                        Some(command) => {
                            let status = if cfg!(target_os = "windows") {
                                std::process::Command::new("cmd")
                                    .args(&["/C", &command])
                                    .status()?
                            } else {
                                std::process::Command::new("sh")
                                    .arg("-c")
                                    .arg(&command)
                                    .status()?
                            };
                            if !status.success() {
                                std::process::exit(status.code().unwrap_or(1));
                            }
                        }
                    }
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

fn print_list<T: Display, I: IntoIterator<Item = T>>(collection: I) {
    for item in collection {
        println!("{}", item);
    }
}

fn parse_version_req(input: Option<String>) -> Result<VersionReq, Error> {
    Ok(match &input {
        None => VersionReq::any(),
        Some(v) if v == "latest" => VersionReq::any(),
        Some(v) => VersionReq::parse(v)?,
    })
}
