use std::sync::Arc;

use aws_sdk_sqs as sqs;
use log::debug;
use serde_json::{json, Value};
use sqs::{error::SdkError, operation::send_message::SendMessageError};

use crate::{
    config::events::EventTopic,
    translate::{ActionContextId, OwnerId},
};

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

impl From<SdkError<SendMessageError>> for EventEmissionError {
    fn from(_value: SdkError<SendMessageError>) -> Self {
        EventEmissionError::ClientError()
    }
}

#[derive(Clone)]
pub struct EventClient {
    client: Arc<sqs::Client>,
}

impl EventClient {
    pub async fn new() -> EventClient {
        let config = ::aws_config::load_from_env().await;
        let client = Arc::new(sqs::Client::new(&config));
        EventClient { client }
    }

    pub fn emit(
        &self,
        topic: &EventTopic,
        owner_id: &Option<OwnerId>,
        action_context_id: &ActionContextId,
        event: &ActionContext,
        page_context: &PageContext,
    ) {
        let payload = json!({
            "owner_id": owner_id,
            "action_context_id": action_context_id,
            "action_context": event,
            "page_context": page_context,
        });
        match serde_json::to_string(&payload) {
            Ok(_payload) => {
                let _client = self.client.clone();
                let _topic = topic.clone();
                tokio::spawn(async move {
                    let resp = _client
                        .send_message()
                        .queue_url(_topic.queue_url.clone())
                        .message_body(_payload)
                        .send()
                        .await;
                    match resp {
                        Ok(_) => {}
                        Err(err) => {
                            log::error!("SQS error: {:?}", err);
                        }
                    }
                });

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
