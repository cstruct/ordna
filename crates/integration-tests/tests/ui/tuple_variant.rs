use ordna_core::config;

#[config]
#[facet(tag = "type")]
enum Backend {
    S3(String),
}

fn main() {}
