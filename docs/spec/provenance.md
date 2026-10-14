# Provenance

A loaded config records where the value of each field came from, in a provenance type `P`
chosen by the config: typically an enum with one variant per kind of source.

r[provenance.mapping]
Registering a source type MUST map the source's errors into the config's error type, and
the detail of each value (where in the source it came from) together with its config path
into `P`.

r[provenance.winner]
For every field set by a source, the loaded config MUST record the origin of the winning
value.

r[provenance.shadowed]
For every field set by a source, the loaded config MUST record the origins of the values
the winner shadowed, most recent first.

r[provenance.unset]
Fields no source set, and paths that are not fields, MUST have no provenance.

r[provenance.list]
The loaded config MUST list the provenance of every set field, sorted by config path.

r[provenance.derived]
A value source MAY be derived from another source; its config path and source kind MUST
be those of the source it was configured by.
