use std::sync::Arc;

use aws_sdk_sqs as sqs;
use futures::{executor::ThreadPool, task::SpawnExt, FutureExt};
use log::debug;
use serde_json::{json, Value};
use sqs::{error::SdkError, operation::send_message::SendMessageError};

use crate::config::events::EventTopic;

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
    pool: ThreadPool,
}

impl EventClient {
    pub async fn new() -> EventClient {
        let config = ::aws_config::load_from_env().await;
        let client = Arc::new(sqs::Client::new(&config));
        let pool = ThreadPool::new().expect("Foo");
        EventClient { client, pool }
    }

    pub fn emit(
        &self,
        topic: &EventTopic,
        event: &ActionContext,
        page_context: &PageContext,
    ) {
        let payload = json!({
            "action_context": event,
            "page_context": page_context,
        });
        match serde_json::to_string(&payload) {
            Ok(_payload) => {
                let res = self
                    .client
                    .send_message()
                    .queue_url(topic.queue_url.clone())
                    .message_body(_payload)
                    .send()
                    .map(|res| match res {
                        Ok(_) => {}
                        Err(err) => {
                            log::error!("SQS error: {:?}", err);
                        }
                    });

                match self.pool.spawn(res) {
                    Ok(_) => {}
                    Err(err) => log::error!("Emission error, unable to enqueue to SQS: {:?}", err),
                }

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
