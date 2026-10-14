# Config paths

A config path identifies one field of a config model, such as `jwt.ttl_seconds`. Sources
provide values for config paths, and the engine reports issues by config path.

## Segments

r[path.segment.syntax]
A config path segment is a struct field name or a map key, and MUST be non-empty and
contain only ASCII alphanumerics, underscores and hyphens.

r[path.segment.static]
Creating a segment from a static name MUST panic when the name is not a valid segment, so
that invalid static names fail at compile time when used in const contexts.

r[path.segment.dynamic]
Creating a segment from a name only known at runtime MUST return an error describing why
the name is invalid: it is empty or contains an invalid character.

## Paths

r[path.non-empty]
A config path MUST have at least one segment.

r[path.dotted]
A config path MUST be displayed as its segments joined by `.`, and parsing that dotted
form MUST produce the same path. Parsing MUST fail if any segment is invalid, including
empty segments.

## Patterns

r[path.pattern]
A config path pattern MUST consist of static segments, matching exactly that segment, and
dynamic segments, matching any segment, displayed as `*`. A path matches a pattern when it
has as many segments and each matches.
