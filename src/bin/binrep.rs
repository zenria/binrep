#![allow(dead_code)]
#![allow(unused_variables)]
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use binrep::binrep::Binrep;
use semver::VersionReq;
use std::fmt::Display;

#[derive(StructOpt)]
struct ListOptions {
    /// artifact name
    artifact_name: Option<String>,
    /// artifact version requirement
    version_req: Option<String>,
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "push")]
    Push,
    #[structopt(name = "pull")]
    Pull,
    #[structopt(name = "ls")]
    List(ListOptions),
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
/// binrep push artifact_name/artifact_version files...
/// binrep pull artifact_name/latest output_dir?
/// binrep pull artifact_name/artifact_version output_dir?
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

        _ => unimplemented!(),
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
