mod client;
mod database;
mod decode;
mod migrations;

pub use database::D1Database;

#[derive(Debug, Clone, Default)]
pub struct D1Config {
    pub account_id: String,
    pub database_id: String,
    pub api_token: String,
    pub base_url: Option<String>,
}

#[cfg(test)]
mod tests;
