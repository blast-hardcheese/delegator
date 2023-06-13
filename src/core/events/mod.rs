use log::debug;
use serde_json::Value;

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
        self,
        event: &ActionContext,
        page_context: &PageContext,
    ) -> Result<(), EventEmissionError> {
        debug!("EventClient.emit({:?}, {:?})", event, page_context);

        Ok(())
    }
}
