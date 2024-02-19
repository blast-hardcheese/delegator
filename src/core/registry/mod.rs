// Registry client.

use crate::model::cryptogram::Cryptogram;
use hashbrown::HashMap;

pub struct RegistryService {
    pub name: String,
    pub spec: String, // Docker spec
    pub pubkey: String,
}

pub struct RegistryResponse {
    pub images: HashMap<String, RegistryService>,
}

pub async fn lookup(_cryptogram: &Cryptogram) -> RegistryResponse {
    RegistryResponse {
        images: HashMap::default(),
    }
}
