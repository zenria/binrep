use crate::extended_exec::{extexec, Line};
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
    pub output_lines: Vec<Line>,
}

pub fn exec<P: AsRef<Path>>(
    artifact: &Artifact,
    pull_directory: P,
    command: &Option<String>,
) -> Result<Option<Vec<Line>>, Error> {
    match command {
        None => Ok(None),
        Some(command) => {
            if command.contains("{}") {
                let mut ret = vec![];
                for file in &artifact.files {
                    let path = path_concat2(&pull_directory, &file.name);
                    let specific_command = command.replace("{}", path.to_string_lossy().borrow());
                    ret.append(&mut exec_command(&specific_command)?);
                }
                Ok(Some(ret))
            } else {
                Ok(Some(exec_command(command.as_str())?))
            }
        }
    }
}

fn exec_command(command: &str) -> Result<Vec<Line>, Error> {
    let status = if cfg!(target_os = "windows") {
        let mut cmd = std::process::Command::new("cmd");

        cmd.args(&["/C", &command]);
        extexec(cmd, true)?
    } else {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(&command);
        extexec(cmd, true)?
    };
    if !status.exit_status.success() {
        Err(ExecutionError {
            command: String::from(command),
            exit_status: status.exit_status,
            output_lines: status.output_lines,
        })?
    } else {
        Ok(status.output_lines)
    }
}

#[cfg(test)]
#[test]
fn test_exec_command() {
    exec_command("echo hello world").unwrap();
    exec_command("echo hello world\necho foo bar").unwrap();
    exec_command("echo hello world && echo foo bar").unwrap();
}
