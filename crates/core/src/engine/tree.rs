use std::collections::BTreeMap;

use facet_core::{Def, Shape, Type, UserType};
use facet_path::{Path, PathStep, ShapeVisitor, VisitDecision, walk_shape};

use crate::{ConfigLayer, ConfigPath, ConfigPathSegment};

/// Collects the config path and layer of every leaf field of `shape`.
///
/// Structs (and options of structs) are treated as branches, everything else is a leaf.
/// A field is secret when it, or any field it is nested in, is marked sensitive.
pub(crate) fn schema_fields(shape: &'static Shape) -> BTreeMap<ConfigPath, ConfigLayer> {
    let mut visitor = ConfigShapeVisitor::default();
    walk_shape(shape, &mut visitor);
    visitor.fields
}

#[derive(Default)]
struct ConfigShapeVisitor {
    fields: BTreeMap<ConfigPath, ConfigLayer>,
}

impl ShapeVisitor for ConfigShapeVisitor {
    fn enter(&mut self, path: &Path, shape: &'static Shape) -> VisitDecision {
        if is_branch(shape) {
            return VisitDecision::Recurse;
        }
        if matches!(path.steps().last(), Some(PathStep::Field(_)))
            && let Some((segments, layer)) = config_path_segments(path)
        {
            self.fields.insert(ConfigPath::new(segments), layer);
        }
        VisitDecision::SkipChildren
    }
}

// r[impl schema.fields.leaves]
fn is_branch(shape: &'static Shape) -> bool {
    match shape.def {
        Def::Option(option) => is_struct(option.t()),
        _ => is_struct(shape),
    }
}

fn is_struct(shape: &'static Shape) -> bool {
    matches!(shape.def, Def::Undefined) && matches!(shape.ty, Type::User(UserType::Struct(_)))
}

/// Converts a facet path into config path segments (struct field names) and the layer of
/// the field it leads to.
///
/// Only struct fields and `Option` wrappers are supported; any other step yields `None`.
pub(crate) fn config_path_segments(path: &Path) -> Option<(Vec<ConfigPathSegment>, ConfigLayer)> {
    let mut segments = Vec::with_capacity(path.len());
    // r[impl schema.layer.public]
    let mut layer = ConfigLayer::Public;
    let mut current_shape = path.shape;
    for step in path.steps() {
        match step {
            PathStep::Field(idx) => {
                let Type::User(UserType::Struct(sd)) = current_shape.ty else {
                    return None;
                };
                let field = sd.fields.get(*idx as usize)?;
                // r[impl schema.fields.names]
                segments.push(ConfigPathSegment::new(field.effective_name()));
                // r[impl schema.layer.secret]
                // r[impl schema.layer.inherit]
                if field.is_sensitive() {
                    layer = ConfigLayer::Secret;
                }
                current_shape = field.shape();
            }
            PathStep::OptionSome => {
                let Def::Option(option) = current_shape.def else {
                    return None;
                };
                current_shape = option.t();
            }
            _ => return None,
        }
    }
    (!segments.is_empty()).then_some((segments, layer))
}
