use ordna_core::config;

#[config]
struct App {
    #[config(secret, secret)]
    password: String,
}

fn main() {}
