use ordna_core::config_tree;

config_tree! {
    #[config(path = "app")]
    struct App {
        name: String,
    }
}

fn main() {}
