use crate::backend::{Backend, BackendError};
use crate::config::S3BackendOpt;
use crate::file_utils;
use failure::_core::time::Duration;
use failure::{Error, Fail};
use futures::future::lazy;
use futures_util::stream::TryStreamExt;
use rusoto_core::{ByteStream, HttpClient, Region, RusotoError};
use rusoto_credential::ProfileProvider;
use rusoto_s3::{
    GetObjectError, GetObjectRequest, PutObjectError, PutObjectRequest, S3Client, StreamingBody, S3,
};
use std::cell::RefCell;
use std::default::Default;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use tokio::runtime::{Handle, Runtime};
use tokio::stream::StreamExt;
use tokio::time::{timeout, Elapsed, Timeout};
use tokio_util::codec;

pub struct S3Backend {
    s3client: S3Client,
    bucket: String,
    request_timeout: Duration,
    runtime: Runtime,
}

#[derive(Fail, Debug)]
pub enum S3BackendError {
    #[fail(display = "No body in response")]
    NoBodyInResponse,
}

impl From<RusotoError<GetObjectError>> for BackendError {
    fn from(e: RusotoError<GetObjectError>) -> Self {
        match e {
            RusotoError::Service(get_error) => match get_error {
                GetObjectError::NoSuchKey(key) => BackendError::ResourceNotFound,
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

impl From<tokio::time::Elapsed> for BackendError {
    fn from(e: Elapsed) -> Self {
        BackendError::Other { cause: e.into() }
    }
}

impl S3Backend {
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
        let runtime = tokio::runtime::Runtime::new()?;
        Ok(Self {
            s3client,
            bucket: opt.bucket.clone(),
            request_timeout: Duration::from_secs(opt.request_timeout_secs.unwrap_or(120)),
            runtime,
        })
    }

    fn get_body(&mut self, path: &str) -> Result<ByteStream, BackendError> {
        let request = self.s3client.get_object(GetObjectRequest {
            bucket: self.bucket.clone(),
            key: path.to_string(),
            ..Default::default() // this one is hacky
        });
        let output = self.execute_with_timeout(request)??;
        match output.body {
            None => Err(S3BackendError::NoBodyInResponse)?,
            Some(body) => Ok(body),
        }
    }

    fn execute_with_timeout<R, F: std::future::Future<Output = R>>(
        &self,
        fut: F,
    ) -> Result<R, Elapsed> {
        // the timeout function needs to be called in the context of a Tokio runtime, thus
        // we use the lazy trick to get our future
        let timeout_future = self
            .runtime
            .handle()
            .block_on(lazy(|_| tokio::time::timeout(self.request_timeout, fut)));
        self.runtime.handle().block_on(timeout_future)
    }

    fn execute<R, F: std::future::Future<Output = R>>(&mut self, fut: F) -> R {
        self.runtime.handle().block_on(fut)
    }

    fn write(&self, path: &str) {}
}

impl Backend for S3Backend {
    fn read_file(&mut self, path: &str) -> Result<String, BackendError> {
        let mut buf = String::new();
        self.get_body(path)?
            .into_blocking_read()
            .read_to_string(&mut buf)?;
        Ok(buf)
    }

    fn create_file(&mut self, path: &str, data: String) -> Result<(), BackendError> {
        let req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: path.to_string(),
            body: Some(data.as_bytes().to_vec().into()),
            acl: Some("bucket-owner-full-control".to_string()),
            ..Default::default()
        };

        self.execute_with_timeout(self.s3client.put_object(req))??;

        Ok(())
    }

    fn push_file(&mut self, local: PathBuf, remote: &str) -> Result<(), BackendError> {
        let meta = std::fs::metadata(&local)?;

        let file = self.execute(tokio::fs::File::open(local))?;
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
        self.execute_with_timeout(self.s3client.put_object(req))??;
        Ok(())
    }

    fn pull_file(&mut self, remote: &str, local: PathBuf) -> Result<(), BackendError> {
        let mut file = File::create(local)?;
        let mut body = self.get_body(remote)?.into_blocking_read();
        std::io::copy(&mut body, &mut file)?;
        Ok(())
    }
}
