use std::io::Read;

pub trait ProgressReporter
where
    Self::Output: Progress,
{
    type Output;

    fn create(name: Option<String>, max: Option<usize>) -> Self::Output;

    fn unnamed_ticker() -> Self::Output {
        Self::create(None, None)
    }
}

pub trait Progress {
    fn inc(&mut self, amount: usize);

    fn tick(&mut self);
}

pub struct ProgressReaderAdapter<R: Read, P: Progress> {
    reader: R,
    progress: P,
}

impl<R: Read, P: Progress> ProgressReaderAdapter<R, P> {
    pub fn new(reader: R, progress: P) -> Self {
        Self { reader, progress }
    }
}

impl<R: Read, P: Progress> Read for ProgressReaderAdapter<R, P> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.reader.read(buf) {
            Ok(bytes_read) => {
                self.progress.inc(bytes_read);
                Ok(bytes_read)
            }
            Err(e) => Err(e),
        }
    }
}

#[pin_project]
pub struct ProgressReaderAsyncAdapter<R: AsyncRead, P: Progress + Send> {
    #[pin]
    reader: R,
    progress: P,
    total_read: usize,
}

impl<R: AsyncRead, P: Progress + Send> ProgressReaderAsyncAdapter<R, P> {
    pub fn new(reader: R, progress: P) -> Self {
        Self {
            reader,
            progress,
            total_read: 0,
        }
    }
}

impl<R: AsyncRead, P: Progress + Send> AsyncRead for ProgressReaderAsyncAdapter<R, P> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        let pinned_reader: Pin<&mut R> = this.reader;
        let poll = pinned_reader.poll_read(cx, buf);
        match &poll {
            Poll::Ready(r) => match r {
                Ok(_) => {
                    let total_read = buf.filled().len();
                    let amount_read = total_read - *this.total_read;
                    this.progress.inc(amount_read);
                    *this.total_read = total_read;
                }
                Err(_) => {}
            },
            _ => {}
        }
        poll
    }
}

mod indicatif;
mod interactive;
mod non_interactive;
mod noop;

use futures::io::Error;
use futures::task::{Context, Poll};
pub use interactive::InteractiveProgressReporter;
pub use noop::NOOPProgress;
use pin_project::pin_project;
use std::io;
use std::pin::Pin;
use tokio::io::AsyncRead;
