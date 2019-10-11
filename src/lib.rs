// dev allows ;)
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

#[macro_use]
extern crate log;

mod backend;
pub mod binrep;
pub mod config;
pub mod config_resolver;
mod crypto;
pub mod exec;
pub mod extended_exec;
pub mod file_utils;
pub mod metadata;
mod path;
mod repository;
pub mod slack;
