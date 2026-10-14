# Config models and schemas

A config model is a Rust type implementing `ConfigModel`. Its schema describes the fields
sources can provide values for, derived from the model's facet shape.

## Fields

r[schema.fields.leaves]
The schema MUST list the path pattern of every leaf field of the model. Structs, options
of branches, maps and internally tagged enums are branches that are walked; every other
field is a leaf, including lists.

r[schema.fields.maps]
A map with scalar keys MUST add a dynamic segment for its keys, followed by the fields of
its values, such as `backends.*.bucket`.

r[schema.fields.enums]
An internally tagged enum whose variants are structs or units MUST provide its tag field
and the fields of every variant at the enum's path. A path shared by several variants MUST
be secret if it is secret in any of them.

r[schema.fields.names]
Each segment of a field's config path MUST be the field's effective name, i.e. its renamed
name when it is renamed.

## Layers

r[schema.layer.public]
A field MUST be public unless it is marked secret.

r[schema.layer.secret]
A field marked sensitive MUST be secret.

r[schema.layer.inherit]
Every field nested in a secret field MUST be secret.

r[schema.layer.nested-leaves]
A leaf whose type contains a field marked sensitive anywhere, such as a list of structs
with a secret field, MUST be secret.
