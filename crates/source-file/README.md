# ordna-source-file

A generic ordna config source for files in any format. Format crates such as
[ordna-source-styx](../source-styx), [ordna-source-json](../source-json),
[ordna-source-toml](../source-toml) and [ordna-source-yaml](../source-yaml) build on it.

```rust
use ordna_source_file::FileSource;
use ordna_source_toml::Toml;

let defaults = FileSource::new("config.toml", Toml);
let secrets = FileSource::new("secrets.toml", Toml).optional().secrets();
```

- A `FileFormat` parses the file into a dynamic value whose root must be an object.
- Keys are walked into config paths: a field gets its whole value (including objects and
  lists), and objects leading towards fields are walked further. Other keys are reported
  as unknown paths.
- A missing file fails the source, unless it is `.optional()`.
- A file holds a single layer: public by default, secret with `.secrets()`. Keep secrets
  in their own file.
- Each value's detail records the file and the dotted key it came from.

Implementing a format:

```rust
impl FileFormat for MyFormat {
    type Error = MyError;

    fn name(&self) -> &'static str {
        "my-format"
    }

    fn parse(&self, contents: &str) -> Result<facet_value::Value, MyError> {
        // ...
    }
}
```
