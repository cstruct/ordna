use ordna_core::config;

#[config]
struct App {
    #[config(rename = "title")]
    name: String,
}

fn main() {}
