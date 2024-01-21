use log::debug;
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    config::events::EventTopic,
    translate::{ActionContextId, OwnerId},
};

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

    pub fn emit(
        &self,
        topic: &EventTopic,
        owner_id: &Option<OwnerId>,
        event_type: &EventType,
        action_context_id: &ActionContextId,
        event: &ActionContext,
        page_context: &PageContext,
    ) {
        let payload = json!({
            "owner_id": owner_id,
            "event_type": event_type,
            "action_context_id": action_context_id,
            "action_context": event,
            "page_context": page_context,
        });
        match serde_json::to_string(&payload) {
            Ok(_payload) => {
                let _topic = topic.clone();

                debug!(
                    "EventClient.emit({:?}, {:?}, {:?})",
                    topic, event, page_context
                );
            }
            Err(err) => {
                log::error!("Unable to encode payload: {:?}", err);
            }
        }
    }
}
