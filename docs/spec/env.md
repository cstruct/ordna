# Environment source

The environment source loads values from environment variables.

## Names

r[env.naming]
The variable for a field MUST be the optional prefix and the field's config path segments,
joined by the separator and uppercased.

r[env.separator]
The separator MUST default to `__` and be configurable.

r[env.case-insensitive]
Variable names MUST be matched case-insensitively.

## Values

r[env.values]
Values MUST be passed on as strings, to be parsed into the field's type when the config is
decoded.

r[env.not-unicode]
A variable matching a field whose value is not valid unicode MUST fail the source.

r[env.detail]
The detail of each value MUST be the name of the variable, as it is set in the
environment.

## Unknown variables

r[env.unprefixed]
Without a prefix, variables that match no field MUST be ignored.

r[env.prefixed-unknown]
With a prefix, a variable starting with the prefix that matches no field MUST be passed on
under the config path derived from its name, so the engine reports it as an unknown path.

r[env.invalid-name]
With a prefix, a variable starting with the prefix that matches no field and can't be
turned into a config path MUST fail the source, unless unknown paths are allowed.

## Secrets

r[env.public]
The environment source MUST always be public, so it can't provide secrets.
