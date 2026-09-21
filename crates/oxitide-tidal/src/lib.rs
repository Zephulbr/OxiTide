pub mod auth;
pub mod catalog;
pub mod client;
pub mod error;
pub mod models;
pub mod pkce;
pub mod token_store;

pub use client::TidalClient;
pub use error::{Error, Result};
