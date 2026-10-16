# Contexts

A context set is a list of config models, contexts, loaded from the same sources. Each
entrypoint of an app loads only the contexts it needs, so a field one entrypoint requires
can be missing for another, while the sources are still checked against every field of
the app.

## Schema

r[contexts.schema]
The schema of a context set MUST hold the fields of every one of its contexts. Contexts
MAY share fields.

r[contexts.schema.secret]
A field that is secret in any context of a set MUST be secret in the set's schema.

## Loading

r[contexts.validate]
Sources MUST be validated against the schema of the context set, whichever contexts are
loaded: a value for a field of a context that isn't loaded MUST NOT be an unknown path.

r[contexts.decode]
Each loaded context MUST be decoded from the values of its own fields only, so values for
fields of other contexts can't make it fail.

r[contexts.decode.collect]
Decode violations of every loaded context MUST be collected and reported together.

r[contexts.membership]
Loading a context that isn't part of the context set MUST be a compile error.

r[contexts.provenance]
The provenance of loaded contexts MUST list the fields of the loaded contexts only.
