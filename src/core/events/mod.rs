use log::debug;
use serde_json::Value;

use crate::translate::EventTopic;

pub type ActionContext = Value;
pub type PageContext = Value;

pub enum EventEmissionError {
    ClientError(),
}

impl From<serde_json::Error> for EventEmissionError {
    fn from(_value: serde_json::Error) -> Self {
        EventEmissionError::ClientError()
    }
}

pub struct EventClient {}

impl EventClient {
    pub async fn new() -> EventClient {
        EventClient {}
    }

    pub fn emit(
        &self,
        topic: &EventTopic,
        event: &ActionContext,
        page_context: &PageContext,
    ) -> Result<(), EventEmissionError> {
        debug!(
            "EventClient.emit({:?}, {:?}, {:?})",
            topic, event, page_context
        );

        Ok(())
    }
}
