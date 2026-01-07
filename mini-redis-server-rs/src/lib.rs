use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod connection;
pub mod file;
pub mod server;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum Request {
    Insert(String, Value),
    Delete(String),
    Read(String),
}

mod tests {
    use super::*;

    #[test]
    fn simple_request_eq() {
        assert_eq!(Request::Read("test".into()), Request::Read("test".into()));
    }
}
