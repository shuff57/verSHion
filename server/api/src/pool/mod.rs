//! The shared pool (server/worker/src/pool) in Rust: items, blobs, circles, moderation, invites, export and the cron
//! stages. Built from the leaves up, so unused code is allowed until the routes land.
#![allow(dead_code)]

pub mod bizdays;
pub mod jsonscan;
pub mod license;
pub mod preread;
pub mod sqlfrag;
pub mod visible;
