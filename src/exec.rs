use crate::file_utils::path_concat2;
use crate::metadata::Artifact;
use core::borrow::Borrow;
use failure::Error;
use failure::Fail;
use std::path::Path;
use std::process::ExitStatus;

#[derive(Fail, Debug)]
#[fail(display = "Command {} returned with status {}", command, exit_status)]
pub struct ExecutionError {
    pub command: String,
    pub exit_status: ExitStatus,
}

pub fn exec<P: AsRef<Path>>(
    artifact: &Artifact,
    pull_directory: P,
    command: &Option<String>,
) -> Result<(), Error> {
    match command {
        None => Ok(()),
        Some(command) => {
            if command.contains("{}") {
                for file in &artifact.files {
                    let path = path_concat2(&pull_directory, &file.name);
                    let specific_command = command.replace("{}", path.to_string_lossy().borrow());
                    exec_command(&specific_command)?;
                }
                Ok(())
            } else {
                exec_command(command.as_str())
            }
        }
    }
}

fn exec_command(command: &str) -> Result<(), Error> {
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
        Err(ExecutionError {
            command: String::from(command),
            exit_status: status,
        })?
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_exec_command() {
    exec_command("echo hello world").unwrap();
    exec_command("echo hello world\necho foo bar").unwrap();
    exec_command("echo hello world && echo foo bar").unwrap();
}
