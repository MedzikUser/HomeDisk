use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Request {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Response {
    pub created: bool,
}