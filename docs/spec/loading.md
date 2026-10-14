# Loading

A config is loaded from an ordered list of sources. Each source provides candidate values
for config paths; the engine validates the candidates, merges them and decodes the result
into the config model.

## Sources

r[load.source.layer]
Every source MUST declare a layer, public or secret. A source's layer MUST default to
public.

r[load.source.error]
When a source fails to load, loading MUST fail with a source error carrying the source's
metadata and its error, mapped into the config's error type.

## Merging

r[load.precedence]
Sources MUST be applied in the order they are added: a value from a later source shadows
the value for the same path from an earlier source.

r[load.decode]
The winning value of every path MUST be decoded into the config model. Fields no source
set MUST be left to the model's defaults, and decoding MUST fail with a decode violation
when a required field is missing or a value doesn't fit its field.

r[load.merge]
Values for different fields of the same struct or map entry MUST be merged into one value,
whichever sources they come from, such as a backend's public fields from a public file and
its credentials from a secrets file.

r[load.decode.strings]
String values MUST be parsed into the field's type when the field isn't a string, such as
`"9090"` into a `u16`.

## Violations

r[load.violations.collect]
All violations of all sources MUST be collected and reported together, instead of failing
at the first one.

r[load.unknown-paths]
A value for a path that is not a field of the schema MUST be reported as an unknown path
violation.

r[load.unknown-paths.allow]
A source that allows unknown paths MUST have its values for unknown paths dropped instead
of reported.

r[load.conflicts]
When a single source provides more than one value for the same path, the values MUST be
reported as a conflict violation.

r[load.layers]
A source MUST only provide values for fields of its own layer. A public source providing a
value for a secret field, or a secret source providing a value for a public field, MUST be
reported as a layer mismatch violation.

r[load.diagnostics.redact]
Values in violations MUST be fully redacted when either the source or the field they
target is secret.
