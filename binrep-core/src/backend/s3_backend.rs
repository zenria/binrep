use crate::backend::{Backend, BackendError, ProgressReporter};
use crate::config::S3BackendOpt;
use crate::file_utils;
use crate::progress::{ProgressReaderAdapter, ProgressReaderAsyncAdapter};
use anyhow::Error;
use atty::Stream;
use futures::future::lazy;
use futures::StreamExt;
use futures::TryStreamExt;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use rusoto_core::{ByteStream, HttpClient, Region, RusotoError};
use rusoto_credential::ProfileProvider;
use rusoto_s3::{
    GetObjectError, GetObjectRequest, PutObjectError, PutObjectRequest, S3Client, StreamingBody, S3,
};
use std::cell::RefCell;
use std::default::Default;
use std::fs::File;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tokio::{
    io::AsyncReadExt,
    time::{timeout, Timeout},
};
use tokio::{
    runtime::{Handle, Runtime},
    time::error::Elapsed,
};
use tokio_io_timeout::TimeoutReader;
use tokio_util::codec;
pub struct S3Backend<T: ProgressReporter> {
    s3client: S3Client,
    bucket: String,
    request_timeout: Duration,
    _progress_reporter: PhantomData<T>,
}

#[derive(thiserror::Error, Debug)]
pub enum S3BackendError {
    #[error("No body in response")]
    NoBodyInResponse,
}

impl From<RusotoError<GetObjectError>> for BackendError {
    fn from(e: RusotoError<GetObjectError>) -> Self {
        match &e {
            RusotoError::Service(get_error) => match get_error {
                GetObjectError::NoSuchKey(key) => BackendError::ResourceNotFound,
                GetObjectError::InvalidObjectState(key) => BackendError::Other { cause: e.into() },
            },
            _ => BackendError::Other { cause: e.into() },
        }
    }
}

impl From<RusotoError<PutObjectError>> for BackendError {
    fn from(e: RusotoError<PutObjectError>) -> Self {
        BackendError::Other { cause: e.into() }
    }
}

impl From<S3BackendError> for BackendError {
    fn from(e: S3BackendError) -> Self {
        BackendError::Other { cause: e.into() }
    }
}

impl From<Elapsed> for BackendError {
    fn from(e: Elapsed) -> Self {
        BackendError::Other { cause: e.into() }
    }
}

impl<T: ProgressReporter> S3Backend<T> {
    pub fn new(opt: &S3BackendOpt) -> Result<Self, Error> {
        let mut profile_provider = ProfileProvider::new()?;
        if let Some(profile) = &opt.profile {
            profile_provider.set_profile(profile.as_str());
        }
        let s3client = S3Client::new_with(
            HttpClient::new()?,
            profile_provider,
            Region::from_str(&opt.region)?,
        );
        Ok(Self {
            s3client,
            bucket: opt.bucket.clone(),
            request_timeout: Duration::from_secs(opt.request_timeout_secs.unwrap_or(120)),
            _progress_reporter: PhantomData,
        })
    }

    async fn get_body(&mut self, path: &str) -> Result<(ByteStream, Option<usize>), BackendError> {
        let request = self.s3client.get_object(GetObjectRequest {
            bucket: self.bucket.clone(),
            key: path.to_string(),
            ..Default::default() // this one is hacky
        });
        let output = self.execute_with_timeout(request).await??;
        let size = output.content_length.map(|i| i as usize);
        match output.body {
            None => Err(S3BackendError::NoBodyInResponse)?,
            Some(body) => Ok((body, size)),
        }
    }

    async fn execute_with_timeout<R, F: std::future::Future<Output = R>>(
        &self,
        fut: F,
    ) -> Result<R, Elapsed> {
        // the timeout function needs to be called in the context of a Tokio runtime, thus
        // we use the lazy trick to get our future
        tokio::time::timeout(self.request_timeout, fut).await
    }
}
#[async_trait::async_trait(?Send)]
impl<T> Backend<T> for S3Backend<T>
where
    T: ProgressReporter,
    T::Output: Send + Sync + 'static,
{
    async fn read_file(&mut self, path: &str) -> Result<String, BackendError> {
        let mut buf = String::new();
        let progress = T::unnamed_ticker();

        let (body, body_size) = self.get_body(path).await?;

        let mut body = ProgressReaderAsyncAdapter::new(body.into_async_read(), progress);

        body.read_to_string(&mut buf).await?;
        Ok(buf)
    }

    async fn create_file(&mut self, path: &str, data: String) -> Result<(), BackendError> {
        let req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: path.to_string(),
            body: Some(data.as_bytes().to_vec().into()),
            acl: Some("bucket-owner-full-control".to_string()),
            ..Default::default()
        };

        self.execute_with_timeout(self.s3client.put_object(req))
            .await??;

        Ok(())
    }

    async fn push_file(&mut self, local: PathBuf, remote: &str) -> Result<(), BackendError> {
        let meta = std::fs::metadata(&local)?;

        let progress = T::create(
            Some(format!("Uploading to {}", remote)),
            Some(meta.len() as usize),
        );
        let file = tokio::fs::File::open(local).await?;
        let file = ProgressReaderAsyncAdapter::new(file, progress);
        let byte_stream =
            codec::FramedRead::new(file, codec::BytesCodec::new()).map_ok(|r| r.freeze());

        let req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: remote.to_string(),
            content_length: Some(meta.len() as i64),
            body: Some(StreamingBody::new(byte_stream)),
            acl: Some("bucket-owner-full-control".to_string()),
            ..Default::default()
        };
        self.execute_with_timeout(self.s3client.put_object(req))
            .await??;
        Ok(())
    }

    async fn pull_file(&mut self, remote: &str, local: PathBuf) -> Result<(), BackendError> {
        let mut file = tokio::fs::File::create(&local).await?;
        let (body, size) = self.get_body(remote).await?;
        let mut body = TimeoutReader::new(body.into_async_read());
        body.set_timeout(Some(Duration::from_secs(30)));
        let body = ProgressReaderAsyncAdapter::new(
            body,
            T::create(Some(format!("downloading {}", remote)), size),
        );

        tokio::io::copy(&mut Box::pin(body), &mut file).await?;

        Ok(())
    }
}
