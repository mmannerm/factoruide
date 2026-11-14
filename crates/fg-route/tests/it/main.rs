//! The planner's integration tests, one binary (one link, one unpacking of the databases).
//! `cargo test --profile fast -p fg-route --test it <name>`.

#![expect(clippy::unwrap_used, reason = "fixtures that cannot be read fail the test")]
mod abilities;
mod acquisition;
mod addon;
mod class_unlocks;
mod common;
mod completion;
mod cooking;
mod data_corrections;
mod draft;
mod edit;
mod profession_trips;
mod required;
mod routes;
mod timeline;
