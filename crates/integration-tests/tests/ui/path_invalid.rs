use ordna_core::config;

#[config]
struct App {
    #[config(path = "api key")]
    name: String,
}

fn main() {}
