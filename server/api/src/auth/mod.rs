//! The auth package (server/auth/src) in Rust. Built bottom-up; the pieces are not routed yet, so unused code is allowed until the handlers land.
#![allow(dead_code)]

pub mod account;
pub mod admin;
pub mod admin_read;
pub mod admin_suspend;
pub mod audit;
pub mod common_passwords;
pub mod config;
pub mod cron;
pub mod crypto;
pub mod device;
pub mod guard;
pub mod http;
pub mod jwt;
pub mod mail;
pub mod maintenance;
pub mod notices;
pub mod password;
pub mod router;
pub mod session;
pub mod signup;
pub mod smtp;
pub mod staff_domains;
pub mod tokens;
pub mod validate;
