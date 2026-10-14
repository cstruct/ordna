# File sources

A file source reads a file and parses it with a file format into a dynamic value, then
walks it into values for config paths.

## Reading

r[file.missing]
A missing file MUST fail the source, unless the file is optional, in which case it MUST
provide no values.

r[file.parse-error]
A file that fails to parse MUST fail the source with the format's error.

r[file.root-object]
The root of a parsed file MUST be an object, otherwise the source MUST fail.

## Walking

r[file.walk]
Object keys MUST be walked into config paths: a key whose path is a field MUST provide the
whole value for that field, including objects and lists, and an object whose path leads
towards fields MUST be walked further.

r[file.unknown-keys]
Keys that are not fields MUST be passed on under their own config path, so the engine
reports them as unknown paths.

r[file.invalid-keys]
A key that is not a valid config path segment MUST fail the source, unless unknown paths
are allowed, in which case it MUST be skipped.

r[file.detail]
The detail of each value MUST be the file's path and the value's dotted key.

## Secrets

r[file.layer]
A file MUST hold a single layer: public by default, or secret when marked as a secrets
file.

## Formats

r[format.styx]
The Styx format MUST parse Styx documents, passing every scalar on as a string.

r[format.json]
The JSON format MUST parse JSON documents.

r[format.toml]
The TOML format MUST parse TOML documents.

r[format.yaml]
The YAML format MUST parse YAML documents.

r[format.mixed]
Files of different formats MUST be usable together as sources of one config.
