use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
struct ListOptions {
    path: Option<String>,
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
}
