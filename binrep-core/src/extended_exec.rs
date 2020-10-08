use std::fmt::Debug;
use std::fmt::Formatter;
use std::io;
use std::io::{Error, Read, Write};
use std::process::{Command, ExitStatus, Stdio};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Type {
    Out,
    Err,
    Cmd,
}

#[derive(Eq, PartialEq)]
pub struct Line {
    pub line_type: Type,
    pub line: Vec<u8>,
}

impl Debug for Line {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{:?}({})",
            self.line_type,
            String::from_utf8_lossy(&self.line)
        )
    }
}

pub struct Output {
    pub exit_status: ExitStatus,
    pub output_lines: Vec<Line>,
}

fn capture_lines<R: Read + Send + 'static, W: Write + Send + 'static>(
    reader: R,
    mut duplicate_stream: Option<W>,
    line_sender: crossbeam::Sender<Line>,
    line_type: Type,
) {
    std::thread::spawn(move || {
        let mut line_buffer = Vec::new();
        for byte in reader.bytes() {
            match byte {
                Ok(byte) => {
                    // I have a byte, forward it if needed
                    // duplicate stream
                    if let Some(writer) = &mut duplicate_stream {
                        let _ = writer.write(&[byte]);
                    };
                    if byte == '\n' as u8 {
                        // new line, sent it to the line channel
                        let mut line = Vec::with_capacity(line_buffer.len());
                        line.append(&mut line_buffer);
                        if let Err(_) = line_sender.send(Line { line, line_type }) {
                            // channel dropped somehow
                            return;
                        }
                    } else {
                        line_buffer.push(byte);
                    }
                }
                Err(_) => break,
            }
        }
        // if there are some remaining bytes, try to send them
        if line_buffer.len() > 0 {
            let _ = line_sender.send(Line {
                line: line_buffer,
                line_type,
            });
        }
    });
}

pub fn extexec(mut command: Command, tee_output_to_std: bool) -> Result<Output, io::Error> {
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
    let (lines_sender, line_receiver) = crossbeam::channel::unbounded();

    lines_sender
        .send(Line {
            line_type: Type::Cmd,
            line: format!("{:?}", command).into_bytes(),
        })
        .unwrap(); // we can safely unwrap here: channels cannot be dropped ;)

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    capture_lines(
        child.stdout.take().unwrap(),
        tee_stdout,
        lines_sender.clone(),
        Type::Out,
    );
    capture_lines(
        child.stderr.take().unwrap(),
        tee_stderr,
        lines_sender,
        Type::Err,
    );
    let exit_status = child.wait().unwrap();
    let output_lines: Vec<_> = line_receiver.iter().collect();
    Ok(Output {
        output_lines,
        exit_status,
    })
}

#[cfg(test)]
mod tests {
    use super::Type::Out;
    use super::*;
    use std::process::Command;

    impl Line {
        fn line(s: &str, line_type: Type) -> Line {
            Line {
                line_type,
                line: s.as_bytes().to_vec(),
            }
        }
        fn out(s: &str) -> Line {
            Line::line(s, Type::Out)
        }
        fn err(s: &str) -> Line {
            Line::line(s, Type::Err)
        }
        fn cmd(s: &str) -> Line {
            Line::line(s, Type::Cmd)
        }
    }

    #[test]
    fn stdout() {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg("echo coucou");

        let output = extexec(cmd, false).unwrap();
        assert_eq!(
            vec![
                Line::cmd(r#""bash" "-c" "echo coucou""#),
                Line::out("coucou")
            ],
            output.output_lines
        );
    }
    #[test]
    fn stderr() {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(">&2 echo coucou");
        let output = extexec(cmd, true).unwrap();
        assert_eq!(
            vec![
                Line::cmd(r#""bash" "-c" ">&2 echo coucou""#),
                Line::err("coucou")
            ],
            output.output_lines
        );
    }

    #[test]
    fn stderrnout() {
        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg("echo foo\n>&2 echo coucou\nsleep 1;echo bar");
        let output = extexec(cmd, true).unwrap();
        assert_eq!(
            vec![
                Line::cmd(r#""bash" "-c" "echo foo\n>&2 echo coucou\nsleep 1;echo bar""#),
                Line::out("foo"),
                Line::err("coucou"),
                Line::out("bar")
            ],
            output.output_lines
        );
        // same without tee output
        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg("echo foo\n>&2 echo coucou\nsleep 1;echo bar");
        let output = extexec(cmd, false).unwrap();
        assert_eq!(
            vec![
                Line::cmd(r#""bash" "-c" "echo foo\n>&2 echo coucou\nsleep 1;echo bar""#),
                Line::out("foo"),
                Line::err("coucou"),
                Line::out("bar")
            ],
            output.output_lines
        );
    }
}
