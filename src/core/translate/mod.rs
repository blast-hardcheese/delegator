use serde_json::Value;

// translate
//
// A poor man's jq.

#[derive(Clone, Debug, PartialEq)]
pub enum Language {
    At(String),                      // .foo
    Focus(String, Box<Language>),    // .foo | ...
    Array(Box<Language>),            // map( ... )
    Object(Vec<(String, Language)>), // { foo: .foo, bar: .bar  }
}

pub fn step(prog: Language, current: &Value) -> Value {
    match prog {
        Language::At(key) => current.get(key).unwrap().clone(),
        Language::Focus(key, next) => step(*next, current.get(key).unwrap()),
        Language::Array(next) => Value::Array(
            current
                .as_array()
                .unwrap()
                .iter()
                .map(|x| step(*next.clone(), x))
                .collect(),
        ),
        Language::Object(pairs) => Value::Object(
            pairs
                .into_iter()
                .map(|(k, v)| (k, step(v, current)))
                .collect(),
        ),
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

    assert_eq!(step(prog, &given), expected);
}
