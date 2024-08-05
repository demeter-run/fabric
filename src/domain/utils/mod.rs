use lazy_static::lazy_static;
use rand::distributions::Alphanumeric;
use rand::Rng;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

lazy_static! {
    static ref ADJECTIVES: Vec<String> = load_adjectives();
    static ref NOUNS: Vec<String> = load_nouns();
}

fn load_adjectives() -> Vec<String> {
    let path = Path::new("src/domain/utils/adjectives.json");
    let file = File::open(path).expect("invalid adjectives file path");
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).expect("adjectives file must be a json array of string")
}

fn load_nouns() -> Vec<String> {
    let path = Path::new("src/domain/utils/nouns.json");
    let file = File::open(path).expect("invalid nouns file path");
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).expect("nouns file must be a json array of string")
}

fn get_random_word(list: &[String]) -> String {
    let mut rng = rand::thread_rng();
    let idx = rng.gen_range(0..list.len());
    list[idx].clone()
}

pub fn get_random_name() -> String {
    let salt: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();

    let adjective = get_random_word(&ADJECTIVES);
    let noun = get_random_word(&NOUNS);

    format!("{adjective}-{noun}-{}", salt.to_lowercase())
}
