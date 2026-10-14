use ordna_core::config;

#[config]
struct App {
    #[config(path = "a.b")]
    name: String,
}

fn main() {}
