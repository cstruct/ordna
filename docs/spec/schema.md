# Config models and schemas

A config model is a Rust type implementing `ConfigModel`. Its schema describes the fields
sources can provide values for, derived from the model's facet shape.

## Fields

r[schema.fields.leaves]
The schema MUST list every leaf field of the model. Structs, and options of structs, are
branches whose fields are walked; every other field is a leaf, including lists and maps.

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
