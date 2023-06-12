use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct EventConfig {
    pub user_action: EventTopic,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct EventTopic {
    pub queue_url: String,
}
