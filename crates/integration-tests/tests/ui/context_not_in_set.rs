use std::convert::Infallible;

use ordna_core::{ConfigBuilder, ConfigPath, SourceFieldPath, config, config_contexts};

#[config]
pub struct Http {
    pub port: u16,
}

#[config]
pub struct Mailer {
    pub host: String,
}

config_contexts!(pub AppContexts = [Http]);

struct Given(ConfigPath);

impl SourceFieldPath for Given {
    fn field_path(&self) -> &ConfigPath {
        &self.0
    }
}

fn main() {
    let _ = ConfigBuilder::<AppContexts, Given, Infallible>::new()
        .source(
            AppContexts::register_source(|never| match never {}, |path, _| Given(path)),
            ordna_core::ValuesSource::new(),
        )
        .load_contexts::<(Http, Mailer)>();
}
