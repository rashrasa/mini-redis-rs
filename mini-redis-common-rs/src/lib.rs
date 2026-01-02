use std::net::SocketAddr;

use serde_json::Value;

pub struct Request {
    info: RequestInfo,
    source: SocketAddr,
}

pub enum RequestInfo {
    Insert(String, Value),
    Read(String),
    Delete(String),
}

pub struct Response {
    request: Request,
}
