use std::{borrow::Cow, fmt};

/// Validated dotted config path such as `jwt.ttl_seconds`.
///
/// ```rust
/// use ordna_core::ConfigPath;
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
        assert!(!segments.is_empty(), "Paths must have at least one segment");
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

// r[impl path.pattern]
/// A config path where segments may be dynamic, matching any segment, such as
/// `backends.*.bucket` for the `bucket` field of every entry of the `backends` map.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfigPathPattern {
    segments: Vec<ConfigPathSegmentPattern>,
}

impl ConfigPathPattern {
    /// Creates a pattern from its segments.
    ///
    /// # Panics
    ///
    /// Panics if `segments` is empty.
    pub fn new(segments: Vec<ConfigPathSegmentPattern>) -> Self {
        assert!(
            !segments.is_empty(),
            "Patterns must have at least one segment"
        );
        Self { segments }
    }

    pub fn segments(&self) -> &[ConfigPathSegmentPattern] {
        &self.segments
    }

    /// Whether `path` matches this pattern.
    pub fn matches(&self, path: &ConfigPath) -> bool {
        path.segments.len() == self.segments.len() && self.matches_start(&path.segments)
    }

    /// Whether `segments` match the start of this pattern without matching all of it,
    /// i.e. lead towards paths matching it.
    pub fn leads_to(&self, segments: &[ConfigPathSegment]) -> bool {
        segments.len() < self.segments.len() && self.matches_start(segments)
    }

    /// The path this pattern matches, if it has no dynamic segments.
    pub fn as_static(&self) -> Option<ConfigPath> {
        let segments = self
            .segments
            .iter()
            .map(|segment| match segment {
                ConfigPathSegmentPattern::Static(segment) => Some(segment.clone()),
                ConfigPathSegmentPattern::Dynamic => None,
            })
            .collect::<Option<_>>()?;
        Some(ConfigPath { segments })
    }

    fn matches_start(&self, segments: &[ConfigPathSegment]) -> bool {
        self.segments
            .iter()
            .zip(segments)
            .all(|(pattern, segment)| match pattern {
                ConfigPathSegmentPattern::Static(expected) => expected == segment,
                ConfigPathSegmentPattern::Dynamic => true,
            })
    }
}

impl fmt::Display for ConfigPathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, segment) in self.segments.iter().enumerate() {
            if i > 0 {
                write!(f, ".")?;
            }
            write!(f, "{segment}")?;
        }
        Ok(())
    }
}

/// One segment of a [`ConfigPathPattern`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigPathSegmentPattern {
    /// Matches exactly this segment, such as a struct field name.
    Static(ConfigPathSegment),
    /// Matches any segment, such as a map key. Displayed as `*`.
    Dynamic,
}

impl fmt::Display for ConfigPathSegmentPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(segment) => segment.fmt(f),
            Self::Dynamic => write!(f, "*"),
        }
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
    use super::{
        ConfigPath, ConfigPathPattern, ConfigPathSegment, ConfigPathSegmentPattern,
        InvalidPathSegment,
    };

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

    // r[verify path.pattern]
    #[test]
    fn patterns_match_any_segment_where_dynamic() {
        let pattern = ConfigPathPattern::new(vec![
            ConfigPathSegmentPattern::Static(ConfigPathSegment::new("backends")),
            ConfigPathSegmentPattern::Dynamic,
            ConfigPathSegmentPattern::Static(ConfigPathSegment::new("bucket")),
        ]);
        let path = |path: &str| ConfigPath::parse(path).unwrap();
        assert_eq!(pattern.to_string(), "backends.*.bucket");
        assert!(pattern.matches(&path("backends.eu-west.bucket")));
        assert!(!pattern.matches(&path("backends.eu-west")));
        assert!(!pattern.matches(&path("backends.eu-west.bucket.x")));
        assert!(!pattern.matches(&path("other.eu-west.bucket")));
        assert!(pattern.leads_to(path("backends.eu-west").segments()));
        assert!(!pattern.leads_to(path("backends.eu-west.bucket").segments()));
        assert_eq!(pattern.as_static(), None);
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
