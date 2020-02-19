use crate::backend::{Backend, BackendError};
use crate::config::S3BackendOpt;
use crate::file_utils;
use failure::_core::time::Duration;
use failure::{Error, Fail};
use futures_fs::{FsPool, ReadOptions};
use rusoto_core::{ByteStream, DefaultCredentialsProvider, HttpClient, Region, RusotoError};
use rusoto_credential::ProfileProvider;
use rusoto_s3::{
    GetObjectError, GetObjectRequest, PutObjectError, PutObjectRequest, S3Client, StreamingBody, S3,
};
use std::default::Default;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;

pub struct S3Backend {
    s3client: S3Client,
    bucket: String,
    request_timeout: Duration,
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

        Ok(Self {
            s3client,
            bucket: opt.bucket.clone(),
            request_timeout: Duration::from_secs(opt.request_timeout_secs.unwrap_or(120)),
        })
    }

    fn get_body(&self, path: &str) -> Result<ByteStream, BackendError> {
        let output = self
            .s3client
            .get_object(GetObjectRequest {
                bucket: self.bucket.clone(),
                key: path.to_string(),
                ..Default::default() // this one is hacky
            })
            .with_timeout(self.request_timeout)
            .sync()?;
        match output.body {
            None => Err(S3BackendError::NoBodyInResponse)?,
            Some(body) => Ok(body),
        }
    }

    fn write(&self, path: &str) {}
}

impl Backend for S3Backend {
    fn read_file(&self, path: &str) -> Result<String, BackendError> {
        let mut buf = String::new();
        self.get_body(path)?
            .into_blocking_read()
            .read_to_string(&mut buf)?;
        Ok(buf)
    }

    fn create_file(&self, path: &str, data: String) -> Result<(), BackendError> {
        let req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: path.to_string(),
            body: Some(data.as_bytes().to_vec().into()),
            acl: Some("bucket-owner-full-control".to_string()),
            ..Default::default()
        };
        let result = self
            .s3client
            .put_object(req)
            .with_timeout(self.request_timeout)
            .sync()?;
        Ok(())
    }

    fn push_file(&self, local: PathBuf, remote: &str) -> Result<(), BackendError> {
        let meta = std::fs::metadata(&local)?;
        let fs = FsPool::default();
        let read_stream = fs.read(local, ReadOptions::default());
        let req = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: remote.to_string(),
            content_length: Some(meta.len() as i64),
            body: Some(StreamingBody::new(read_stream)),
            acl: Some("bucket-owner-full-control".to_string()),
            ..Default::default()
        };
        self.s3client
            .put_object(req)
            .with_timeout(self.request_timeout)
            .sync()?;
        Ok(())
    }

    fn pull_file(&self, remote: &str, local: PathBuf) -> Result<(), BackendError> {
        let mut file = File::create(local)?;
        let mut body = self.get_body(remote)?.into_blocking_read();
        std::io::copy(&mut body, &mut file)?;
        Ok(())
    }
}
