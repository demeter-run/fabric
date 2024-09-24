use std::collections::BTreeMap;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::{
    CustomResourceDefinition, JSONSchemaProps,
};
use rand::distributions::Alphanumeric;
use rand::Rng;

const ADJECTIVES: &str = include_str!("adjectives");
const NOUNS: &str = include_str!("nouns");

fn get_random_word(list: Vec<&str>) -> String {
    let mut rng = rand::thread_rng();
    let idx = rng.gen_range(0..list.len());
    list[idx].to_string()
}

pub fn get_random_name() -> String {
    let salt = get_random_salt();
    let adjective = get_random_word(ADJECTIVES.lines().collect());
    let noun = get_random_word(NOUNS.lines().collect());

    format!("{adjective}-{noun}-{salt}")
}

pub fn get_random_salt() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .take(6)
        .map(char::from)
        .collect()
}

pub fn get_schema_from_crd(
    crd: &CustomResourceDefinition,
    field: &str,
) -> Option<BTreeMap<String, JSONSchemaProps>> {
    let version = crd.spec.versions.last()?;
    let schema = version.schema.clone()?.open_api_v3_schema?.properties?;
    schema.get(field)?.properties.clone()
}
