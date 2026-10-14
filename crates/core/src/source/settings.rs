use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone)]
pub struct ConfigSourceSettings {
    pub layer: ConfigLayer,
    pub allow_unknown_paths: bool,
}

impl ConfigSourceSettings {
    pub fn builder() -> ConfigSourceSettingsBuilder {
        ConfigSourceSettingsBuilder::default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConfigSourceSettingsBuilder {
    pub layer: ConfigLayer,
    pub allow_unknown_paths: bool,
}

impl ConfigSourceSettingsBuilder {
    pub fn public(mut self) -> Self {
        self.layer = ConfigLayer::Public;
        self
    }

    pub fn secrets(mut self) -> Self {
        self.layer = ConfigLayer::Secret;
        self
    }

    pub fn allow_unknown_paths(mut self, allow: bool) -> Self {
        self.allow_unknown_paths = allow;
        self
    }

    pub fn build(self) -> ConfigSourceSettings {
        ConfigSourceSettings {
            layer: self.layer,
            allow_unknown_paths: self.allow_unknown_paths,
        }
    }
}

// r[impl load.source.layer]
/// The different config source layers.
///
/// **Public**: is the default layer holding unsesitive data that most sources can load into.
/// **Secret**: is the layer holds sensitive data that can not be loadable from insecure sources.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigLayer {
    #[default]
    Public,
    Secret,
}

impl Display for ConfigLayer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLayer::Public => write!(f, "public"),
            ConfigLayer::Secret => write!(f, "secret"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigLayer, ConfigSourceSettings};

    // r[verify load.source.layer]
    #[test]
    fn sources_are_public_by_default() {
        assert_eq!(
            ConfigSourceSettings::builder().build().layer,
            ConfigLayer::Public
        );
        assert_eq!(
            ConfigSourceSettings::builder().secrets().build().layer,
            ConfigLayer::Secret
        );
    }
}
