#![allow(dead_code)]
#![allow(unused_variables)]

use std::io;
use std::io::{Read, Write};
use std::path::Path;

pub mod backend;
pub mod config;
pub mod metadata;

pub type Version = String;

pub trait Repository {
    fn init(&self);

    fn add_artifact<P: AsRef<Path>>(&self, name: &str, version: &Version, files: &[P]);
}
