pub mod deserialize;
pub mod parse;

use crate::config::events::EventTopic;
use crate::events::{EventClient, PageContext};

use std::borrow::{Borrow, BorrowMut};
use std::fmt::Display;
use std::sync::{Arc, Mutex};

use hashbrown::HashMap;
use serde_json::{Map, Value};

pub struct TranslateContext {
    client: Option<Arc<EventClient>>,
}

impl TranslateContext {
    pub fn noop() -> TranslateContext {
        TranslateContext { client: None }
    }

    pub fn build(client: Arc<EventClient>) -> TranslateContext {
        TranslateContext {
            client: Some(client),
        }
    }
}

// translate
//
// A poor man's jq.

#[derive(Clone, Debug, PartialEq)]
pub enum Language {
    At(String),                      // .foo
    Focus(String, Box<Language>),    // .foo | ...
    Array(Box<Language>),            // map( ... )
    Object(Vec<(String, Language)>), // { foo: .foo, bar: .bar  }
    Splat(Vec<Language>),            // .foo, .bar
    Set(String),                     // ... | set("foo")
    Get(String),                     // get("bar") | ...
    Const(Value),                    // const(...)
    Identity,
    EmitEvent(EventTopic, PageContext),
}

#[derive(Debug)]
pub struct StepError {
    history: Vec<String>,
    choices: Option<Value>,
}

impl StepError {
    fn new(root: String) -> StepError {
        StepError {
            history: vec![root],
            choices: None,
        }
    }

    fn prepend_history(mut self, before: String) -> StepError {
        self.history.insert(0, before);
        self
    }
}

impl std::error::Error for StepError {}

impl Display for StepError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "StepError({}, {})",
            self.history.join(", "),
            self.choices
                .clone()
                .map_or(String::from("[]"), |cs| format!("{}", cs))
        )
    }
}

pub type State = Arc<Mutex<HashMap<String, Arc<Value>>>>;

pub fn make_state() -> State {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn step(
    ctx: &TranslateContext,
    prog: &Language,
    current: &Value,
    state: State,
) -> Result<Value, StepError> {
    match prog {
        Language::At(key) => Ok(current
            .get(key)
            .ok_or_else(|| StepError {
                history: vec![key.clone()],
                choices: current
                    .as_object()
                    .map(|o| Value::Array(o.keys().map(|x| Value::String(x.to_owned())).collect())),
            })?
            .clone()),
        Language::Focus(key, next) => step(
            ctx,
            next,
            current.get(key).ok_or_else(|| StepError {
                history: vec![key.clone()],
                choices: current
                    .as_object()
                    .map(|o| Value::Array(o.keys().map(|x| Value::String(x.to_owned())).collect())),
            })?,
            state,
        )
        .map_err(|se| se.prepend_history(key.clone())),
        Language::Array(next) => Ok(Value::Array(
            current
                .as_array()
                .ok_or_else(|| StepError::new(String::from("<Not an array>")))?
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    step(ctx, next, x, state.clone())
                        .map_err(|se| se.prepend_history(format!("[{}]", i)))
                })
                .collect::<Result<Vec<Value>, StepError>>()?,
        )),
        Language::Object(pairs) => Ok(Value::Object(
            pairs
                .iter()
                .map(|(k, v)| step(ctx, v, current, state.clone()).map(|v| (k.clone(), v)))
                .collect::<Result<Map<String, Value>, StepError>>()?,
        )),
        Language::Splat(each) => {
            let result = each
                .iter()
                .map(|next| step(ctx, next, current, state.clone()))
                .collect::<Result<Vec<Value>, StepError>>()?;
            Ok(result.last().unwrap().clone())
        }
        Language::Set(into) => {
            state
                .lock()
                .map_err(|_e| StepError::new(format!("Set({})", into)))?
                .borrow_mut()
                .insert(into.clone(), Arc::new(current.clone()));
            Ok(current.clone())
        }
        Language::Get(from) => {
            let mutex = state
                .lock()
                .map_err(|_e| StepError::new(String::from("<Unable to acquire mutex lock>")))?;
            let _state = mutex.borrow();
            let needle = _state
                .get(from)
                .ok_or_else(|| StepError::new(format!("Get({})", from)))?;
            Ok((**needle).clone())
        }
        Language::Const(value) => Ok(value.clone()),
        Language::Identity => Ok(current.clone()),
        Language::EmitEvent(topic, page_context) => {
            if let Some(client) = &ctx.client {
                client.emit(topic, current, page_context);
            }
            Ok(current.clone())
        }
    }
}

#[test]
fn translate_error_at() {
    let ctx = TranslateContext::noop();
    use serde_json::json;
    let prog = Language::At(String::from("foo"));

    let given = json!({ "bar": "baz" });
    if let Some(StepError {
        choices: _,
        history,
    }) = step(&ctx, &prog, &given, make_state()).err()
    {
        assert_eq!(history, vec!["foo"]);
    }
}

#[test]
fn translate_error_array() {
    let ctx = TranslateContext::noop();
    use serde_json::json;
    let prog = Language::Array(Box::new(Language::At(String::from("foo"))));

    let given = json!([{ "bar": "baz" }]);
    if let Some(StepError {
        choices: _,
        history,
    }) = step(&ctx, &prog, &given, make_state()).err()
    {
        assert_eq!(history, vec!["[0]", "foo"]);
    }
}

#[test]
fn translate_error_focus() {
    let ctx = TranslateContext::noop();
    use serde_json::json;
    let prog = Language::Focus(
        String::from("foo"),
        Box::new(Language::At(String::from("bar"))),
    );

    let given = json!({ "baz": "blix" });
    if let Some(StepError {
        choices: _,
        history,
    }) = step(&ctx, &prog, &given, make_state()).err()
    {
        assert_eq!(history, vec!["foo"]);
    }
}

#[test]
fn translate_error_object() {
    let ctx = TranslateContext::noop();
    use serde_json::json;
    let prog = Language::Object(vec![
        (String::from("foo"), Language::At(String::from("foo"))),
        (String::from("bar"), Language::At(String::from("bar"))),
    ]);

    let given = json!({ "foo": "foo" });
    if let Some(StepError {
        choices: _,
        history,
    }) = step(&ctx, &prog, &given, make_state()).err()
    {
        assert_eq!(history, vec!["bar"]);
    }
}

#[test]
fn translate_test() {
    let ctx = TranslateContext::noop();
    use serde_json::json;
    let prog = Language::Focus(
        String::from("results"),
        Box::new(Language::Object(vec![(
            String::from("ids"),
            Language::Array(Box::new(Language::At(String::from("product_variant_id")))),
        )])),
    );

    let given = json!({ "q": "Foo", "results": [{"product_variant_id": "12313bb7-6068-4ec9-ac49-3e834181f127"}] });
    let expected = json!({ "ids": [ "12313bb7-6068-4ec9-ac49-3e834181f127" ] });

    assert_eq!(step(&ctx, &prog, &given, make_state()).unwrap(), expected);
}
