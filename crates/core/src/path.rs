use std::{borrow::Cow, fmt};

/// Validated dotted config path such as `jwt.ttl_seconds`.
///
/// ```rust
/// use ::core::ConfigPath;
///
/// let path = ConfigPath::parse("jwt.ttl_seconds").unwrap();
/// assert_eq!(path.to_string(), "jwt.ttl_seconds");
/// assert!(ConfigPath::parse("jwt..ttl_seconds").is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfigPath {
    segments: Vec<ConfigPathSegment>,
}

impl ConfigPath {
    // r[impl path.non-empty]
    /// Creates a config path from its segments.
    ///
    /// # Panics
    ///
    /// Panics if `segments` is empty.
    pub const fn new(segments: Vec<ConfigPathSegment>) -> Self {
        assert!(segments.len() > 0, "Paths must have at least one segment");
        Self { segments }
    }

    // r[impl path.dotted]
    /// Parses a dotted path such as `jwt.ttl_seconds`.
    pub fn parse(path: &str) -> Result<Self, InvalidPathSegment> {
        let segments = path
            .split('.')
            .map(|segment| ConfigPathSegment::parse(segment.to_string()))
            .collect::<Result<_, _>>()?;
        Ok(Self { segments })
    }

    /// Returns a slice of the config path segments.
    pub fn segments(&self) -> &[ConfigPathSegment] {
        &self.segments
    }
}

impl std::str::FromStr for ConfigPath {
    type Err = InvalidPathSegment;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        Self::parse(path)
    }
}

// r[impl path.dotted]
impl fmt::Display for ConfigPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.segments[0])?;
        for segment in &self.segments[1..] {
            write!(f, ".{}", segment)?;
        }
        Ok(())
    }
}

/// One segment of a [`ConfigPath`], such as `jwt` or `ttl_seconds`.
///
/// A segment is a struct field name or a map key, and contains only ASCII alphanumerics,
/// underscores and hyphens. Use [`ConfigPathSegment::new`] for static names
/// (validated at compile time in const contexts) and [`ConfigPathSegment::parse`] for
/// names only known at runtime.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfigPathSegment(Cow<'static, str>);

impl ConfigPathSegment {
    // r[impl path.segment.static]
    /// Creates a segment from a static name.
    ///
    /// # Panics
    ///
    /// Panics if `segment` is not a valid segment.
    pub const fn new(segment: &'static str) -> Self {
        match validate_segment(segment) {
            Ok(()) => Self(Cow::Borrowed(segment)),
            Err(InvalidPathSegment::Empty) => panic!("path segments should not be empty"),
            Err(InvalidPathSegment::InvalidCharacter) => panic!(
                "path segments should contain only alphanumeric characters, underscores or hyphens"
            ),
        }
    }

    // r[impl path.segment.dynamic]
    /// Creates a segment from a name only known at runtime.
    pub fn parse(segment: impl Into<Cow<'static, str>>) -> Result<Self, InvalidPathSegment> {
        let segment = segment.into();
        validate_segment(&segment)?;
        Ok(Self(segment))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a string is not a valid [`ConfigPathSegment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidPathSegment {
    Empty,
    InvalidCharacter,
}

impl fmt::Display for InvalidPathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "path segments should not be empty"),
            Self::InvalidCharacter => write!(
                f,
                "path segments should contain only alphanumeric characters, underscores or hyphens"
            ),
        }
    }
}

impl std::error::Error for InvalidPathSegment {}

// r[impl path.segment.syntax]
const fn validate_segment(segment: &str) -> Result<(), InvalidPathSegment> {
    let bytes = segment.as_bytes();
    if bytes.is_empty() {
        return Err(InvalidPathSegment::Empty);
    }
    let mut i = 0;
    while i < bytes.len() {
        if !(bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-') {
            return Err(InvalidPathSegment::InvalidCharacter);
        }
        i += 1;
    }
    Ok(())
}

impl fmt::Display for ConfigPathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigPath, ConfigPathSegment, InvalidPathSegment};

    // r[verify path.segment.syntax]
    // r[verify path.segment.dynamic]
    #[test]
    fn dynamic_segments_are_validated() {
        let segment = ConfigPathSegment::parse(String::from("ttl_seconds")).unwrap();
        assert_eq!(segment, ConfigPathSegment::new("ttl_seconds"));
        assert_eq!(ConfigPathSegment::parse(""), Err(InvalidPathSegment::Empty));
        // Map keys such as `eu-west` or `2024` are valid segments.
        assert!(ConfigPathSegment::parse("eu-west").is_ok());
        assert!(ConfigPathSegment::parse("2024").is_ok());
        for invalid in ["a.b", "a b", "a/b", "*", "é"] {
            assert_eq!(
                ConfigPathSegment::parse(invalid),
                Err(InvalidPathSegment::InvalidCharacter),
                "{invalid}"
            );
        }
    }

    // r[verify path.segment.static]
    #[test]
    #[should_panic(expected = "path segments should contain only alphanumeric characters")]
    fn invalid_static_segments_panic() {
        let name = "a.b";
        ConfigPathSegment::new(name);
    }

    // r[verify path.non-empty]
    // r[verify path.dotted]
    #[test]
    fn paths_parse_from_dotted_strings() {
        let path = ConfigPath::parse("jwt.ttl_seconds").unwrap();
        assert_eq!(path.to_string(), "jwt.ttl_seconds");
        assert_eq!(path.segments().len(), 2);
        assert_eq!(ConfigPath::parse(""), Err(InvalidPathSegment::Empty));
        assert_eq!(ConfigPath::parse("a..b"), Err(InvalidPathSegment::Empty));
    }
}
