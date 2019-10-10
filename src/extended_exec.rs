use std::io;
use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};

pub struct Output {
    pub exit_status: ExitStatus,
    pub stderr: Vec<u8>,
    pub stdout: Vec<u8>,
}

fn capture_output<T: Read + Send + 'static, W: Write + Send + 'static>(
    std_err_out: T,
    mut duplicate_output: Option<W>,
) -> crossbeam::Receiver<u8> {
    let (sender, receiver) = crossbeam::channel::unbounded::<u8>();
    std::thread::spawn(move || {
        for byte in std_err_out.bytes() {
            if let Ok(byte) = byte {
                if let Err(_) = sender.send(byte) {
                    // channel dropped:
                    return;
                }
                // duplicate stream
                if let Some(writer) = &mut duplicate_output {
                    let _ = writer.write(&[byte]);
                };
            } else {
                // error reading stdout, ignore
                return;
            }
        }
    });
    receiver
}

pub fn extexec(mut command: Command, tee_output_to_std: bool) -> Result<Output, io::Error> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let tee_stderr = if tee_output_to_std {
        Some(std::io::stderr())
    } else {
        None
    };
    let tee_stdout = if tee_output_to_std {
        Some(std::io::stdout())
    } else {
        None
    };

    let stdout_receiver = capture_output(child.stdout.take().unwrap(), tee_stdout);
    let stderr_receiver = capture_output(child.stderr.take().unwrap(), tee_stderr);
    let exit_status = child.wait().unwrap();
    let stdout: Vec<u8> = stdout_receiver.iter().collect();
    let stderr: Vec<u8> = stderr_receiver.iter().collect();
    Ok(Output {
        stderr,
        stdout,
        exit_status,
    })
}

#[cfg(test)]
mod tests {
    use crate::extended_exec::extexec;
    use std::process::Command;

    #[test]
    fn stdout() {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg("echo coucou");

        let output = extexec(cmd, false).unwrap();
        assert_eq!("coucou\n".as_bytes(), output.stdout.as_slice());
    }
    #[test]
    fn stderr() {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(">&2 echo coucou");
        let output = extexec(cmd, true).unwrap();
        assert_eq!("".as_bytes(), output.stdout.as_slice());
        assert_eq!("coucou\n".as_bytes(), output.stderr.as_slice());
    }
}
