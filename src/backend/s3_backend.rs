use crate::backend::Backend;
use crate::config::S3BackendOpt;
use crate::file_utils;
use failure::{Error, Fail};
use rusoto_core::{DefaultCredentialsProvider, HttpClient, Region};
use rusoto_credential::ProfileProvider;
use rusoto_s3::{GetObjectRequest, S3Client, S3};
use std::io::Read;
use std::path::PathBuf;

pub struct S3Backend {
    s3client: S3Client,
    bucket: String,
}

#[derive(Fail, Debug)]
pub enum S3BackendError {
    #[fail(display = "No body in response")]
    NoBodyInResponse,
}

impl S3Backend {
    pub fn new(opt: &S3BackendOpt) -> Result<Self, Error> {
        let mut profile_provider = ProfileProvider::new()?;
        if let Some(profile) = &opt.profile {
            profile_provider.set_profile(profile.as_str());
        }

        let s3client = S3Client::new_with(HttpClient::new()?, profile_provider, opt.region.clone());

        Ok(Self {
            s3client,
            bucket: opt.bucket.clone(),
        })
    }
}

impl Backend for S3Backend {
    fn read_file(&self, path: &str) -> Result<String, Error> {
        // OMFG
        let output = self
            .s3client
            .get_object(GetObjectRequest {
                bucket: self.bucket.clone(),
                if_match: None,
                if_modified_since: None,
                if_none_match: None,
                if_unmodified_since: None,
                key: path.into(),
                part_number: None,
                range: None,
                request_payer: None,
                response_cache_control: None,
                response_content_disposition: None,
                response_content_encoding: None,
                response_content_language: None,
                response_content_type: None,
                response_expires: None,
                sse_customer_algorithm: None,
                sse_customer_key: None,
                sse_customer_key_md5: None,
                version_id: None,
            })
            .sync()?;
        match output.body {
            None => Err(S3BackendError::NoBodyInResponse)?,
            Some(body) => {
                let mut buf = String::new();
                body.into_blocking_read().read_to_string(&mut buf)?;
                Ok(buf)
            }
        }
    }

    fn create_file(&self, path: &str, data: String) -> Result<(), Error> {
        unimplemented!()
    }

    fn push_file(&self, local: PathBuf, remote: &str) -> Result<(), Error> {
        unimplemented!()
    }

    fn pull_file(&self, remote: &str, local: PathBuf) -> Result<(), Error> {
        unimplemented!()
    }
}
