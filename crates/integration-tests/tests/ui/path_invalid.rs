use ordna_core::config;

#[config]
struct App {
    #[config(path = "1st")]
    name: String,
}

fn main() {}
