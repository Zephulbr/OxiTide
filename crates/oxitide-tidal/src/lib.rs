pub mod auth;
pub mod catalog;
pub mod client;
pub mod error;
pub mod pages;
pub mod library;
pub mod models;
pub mod pkce;
pub mod playlists;
pub mod search;
pub mod stream;
pub mod token_store;

pub use client::TidalClient;
pub use error::{Error, Result};
