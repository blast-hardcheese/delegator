use std::{
    collections::hash_map::DefaultHasher,
    hash::Hasher,
    time::{Duration, Instant},
};

use hashbrown::HashMap;
use serde_json::Value;
use tokio::sync::Mutex;

pub type Ttl = Duration;

pub struct MemoizationCache {
    cache: HashMap<String, (Instant, Ttl, Value)>,
}

impl MemoizationCache {
    pub fn new() -> Mutex<MemoizationCache> {
        Mutex::new(MemoizationCache::empty())
    }

    pub fn empty() -> MemoizationCache {
        MemoizationCache {
            cache: HashMap::new(),
        }
    }

    pub fn get(&self, key: &String) -> Option<&Value> {
        self.cache.get(key).and_then(|(cached_at, ttl, value)| {
            if cached_at.elapsed().gt(ttl) {
                None
            } else {
                Some(value)
            }
        })
    }

    pub fn insert(&mut self, key: String, value: Value, ttl: Ttl) -> Value {
        self.cache.insert(key, (Instant::now(), ttl, value.clone()));
        value
    }
}

impl Default for MemoizationCache {
    fn default() -> Self {
        Self::empty()
    }
}

pub fn hash_value(value: &Value) -> String {
    let mut hasher = DefaultHasher::new();

    fn step(hasher: &mut DefaultHasher, x: &Value) {
        match x {
            Value::String(s) => {
                let hashable = s.clone().into_bytes();
                Hasher::write(hasher, &hashable)
            }
            Value::Null => Hasher::write_u8(hasher, 0),
            Value::Bool(v) => Hasher::write_u8(hasher, if v.to_owned() { 1 } else { 0 }),
            Value::Number(n) => {
                let _u64 = n.as_u64().map(|x| x.to_be_bytes());
                let _i64 = n.as_i64().map(|x| x.to_be_bytes());
                let _f64 = n.as_f64().map(|x| x.to_be_bytes());
                let b = _u64
                    .or(_i64)
                    .or(_f64)
                    .expect("Somehow we were trying to hash an unrepresentable number");
                Hasher::write(hasher, &b)
            }
            Value::Array(vec) => {
                for x in vec {
                    step(hasher, x);
                }
            }
            Value::Object(hm) => {
                let mut keys: Vec<&String> = hm.keys().collect();
                keys.sort();
                for x in keys {
                    let k = x.clone().into_bytes();
                    Hasher::write(hasher, &k);
                    step(
                        hasher,
                        hm.get(x).expect("Object changed out from under us!"),
                    );
                }
            }
        }
    }

    step(&mut hasher, value);
    format!("{:x}", hasher.finish())
}
