pub mod auth;
pub mod catalog;
pub mod client;
pub mod error;
pub mod folders;
pub mod library;
pub mod lyrics;
pub mod models;
pub mod pages;
pub mod pkce;
pub mod playlists;
pub mod purchases;
pub mod search;
pub mod stream;
pub mod token_store;

pub use client::TidalClient;
pub use error::{Error, Result};
