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
    let salt: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .take(6)
        .map(char::from)
        .collect();

    let adjective = get_random_word(ADJECTIVES.lines().collect());
    let noun = get_random_word(NOUNS.lines().collect());

    format!("{adjective}-{noun}-{salt}")
}
