use log::debug;
use serde::Serialize;
use serde_json::Value;

use crate::config::events::EventTopic;

pub type ActionContext = Value;
pub type PageContext = Value;

pub enum EventEmissionError {
    ClientError(),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum EventType {
    #[serde(rename = "search")]
    Search,
    #[serde(rename = "search_result")]
    SearchResult,
}

impl From<serde_json::Error> for EventEmissionError {
    fn from(_value: serde_json::Error) -> Self {
        EventEmissionError::ClientError()
    }
}

#[derive(Clone)]
pub struct EventClient {}

impl EventClient {
    pub async fn new() -> EventClient {
        EventClient {}
    }

    pub fn emit(&self, topic: &EventTopic, event: &ActionContext) {
        match serde_json::to_string(&event) {
            Ok(_payload) => {
                let _topic = topic.clone();

                debug!("EventClient.emit({:?}, {:?})", topic, event);
            }
            Err(err) => {
                log::error!("Unable to encode payload: {:?}", err);
            }
        }
    }
}
