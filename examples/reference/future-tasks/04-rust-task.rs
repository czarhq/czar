// tasks/extract.rs
use czar::task;
use serde::{Deserialize, Serialize};
use crate::extract::fetch_users;

#[derive(Default, Deserialize)]
struct Input {
    #[serde(default = "default_api_url")]
    api_url: String,
}

fn default_api_url() -> String {
    "https://api.example.com".into()
}

#[derive(Default, Serialize, Deserialize)]
struct User {
    id: i64,
    name: String,
    email: String,
}

#[derive(Default, Serialize)]
struct Output {
    users: Vec<User>,
}

#[task]
fn main(input: Input) -> czar::Result<Output> {
    let users = fetch_users(&input.api_url)?;
    Ok(Output { users })
}
