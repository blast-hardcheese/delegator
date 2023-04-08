pub mod deserialize;
pub mod parse;

use std::borrow::{Borrow, BorrowMut};
use std::fmt::Display;
use std::sync::{Arc, Mutex};

use hashbrown::HashMap;
use serde_json::{Map, Value};

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
}

#[derive(Debug)]
pub struct StepError {
    history: Vec<String>,
}

impl Display for StepError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "StepError({})", self.history.join(", "))
    }
}

pub type State = Arc<Mutex<HashMap<String, Arc<Value>>>>;

pub fn make_state() -> State {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn step(prog: &Language, current: &Value, state: State) -> Result<Value, StepError> {
    match prog {
        Language::At(key) => Ok(current
            .get(key)
            .ok_or_else(|| StepError { history: vec![key.clone()] })?
            .clone()),
        Language::Focus(key, next) => step(
            next,
            current.get(key).ok_or_else(|| StepError {
                history: vec![key.clone()],
            })?,
            state,
        )
        .map_err(|StepError { mut history }| StepError {
            history: {
                history.insert(0, key.clone());
                history
            },
        }),
        Language::Array(next) => Ok(Value::Array(
            current
                .as_array()
                .ok_or_else(|| StepError {
                    history: vec![String::from("<Not an array>")],
                })?
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    step(next, x, state.clone()).map_err(|StepError { mut history }| {
                        StepError {
                            history: {
                                history.insert(0, format!("[{}]", i));
                                history
                            },
                        }
                    })
                })
                .collect::<Result<Vec<Value>, StepError>>()?,
        )),
        Language::Object(pairs) => Ok(Value::Object(
            pairs
                .iter()
                .map(|(k, v)| step(v, current, state.clone()).map(|v| (k.clone(), v)))
                .collect::<Result<Map<String, Value>, StepError>>()?,
        )),
        Language::Splat(each) => {
            let result = each
                .iter()
                .map(|next| step(next, current, state.clone()))
                .collect::<Result<Vec<Value>, StepError>>()?;
            Ok(result.last().unwrap().clone())
        }
        Language::Set(into) => {
            state
                .lock()
                .map_err(|_e| StepError { history: vec![ format!("Set({})", into) ] })?
                .borrow_mut()
                .insert(into.clone(), Arc::new(current.clone()));
            Ok(current.clone())
        }
        Language::Get(from) => {
            let mutex = state.lock().map_err(|_e| StepError { history: vec![ String::from("<Unable to acquire mutex lock>") ] })?;
            let _state = mutex.borrow();
            let needle = _state
                .get(from)
                .ok_or_else(|| StepError { history: vec![ format!("Get({})", from) ] })?;
            Ok((**needle).clone())
        }
    }
}

#[test]
fn translate_error_at() {
    use serde_json::json;
    let prog = Language::At(String::from("foo"));

    let given = json!({ "bar": "baz" });
    if let Some(StepError { history }) = step(&prog, &given, make_state()).err() {
        assert_eq!(history, vec!["foo"]);
    }
}

#[test]
fn translate_error_array() {
    use serde_json::json;
    let prog = Language::Array(Box::new(Language::At(String::from("foo"))));

    let given = json!([{ "bar": "baz" }]);
    if let Some(StepError { history }) = step(&prog, &given, make_state()).err() {
        assert_eq!(history, vec!["[0]", "foo"]);
    }
}

#[test]
fn translate_error_focus() {
    use serde_json::json;
    let prog = Language::Focus(
        String::from("foo"),
        Box::new(Language::At(String::from("bar"))),
    );

    let given = json!({ "baz": "blix" });
    if let Some(StepError { history }) = step(&prog, &given, make_state()).err() {
        assert_eq!(history, vec!["foo"]);
    }
}

#[test]
fn translate_error_object() {
    use serde_json::json;
    let prog = Language::Object(vec![
        (String::from("foo"), Language::At(String::from("foo"))),
        (String::from("bar"), Language::At(String::from("bar"))),
    ]);

    let given = json!({ "foo": "foo" });
    if let Some(StepError { history }) = step(&prog, &given, make_state()).err() {
        assert_eq!(history, vec!["bar"]);
    }
}

#[test]
fn translate_test() {
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

    assert_eq!(step(&prog, &given, make_state()).unwrap(), expected);
}
