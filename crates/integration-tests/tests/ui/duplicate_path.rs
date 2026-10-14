use ordna_core::config;

#[config]
struct App {
    #[config(path = "name")]
    title: String,
    name: String,
}

fn main() {}
