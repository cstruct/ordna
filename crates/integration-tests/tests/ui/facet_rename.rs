use ordna_core::config;

#[config]
struct App {
    #[facet(rename = "title")]
    name: String,
}

fn main() {}
