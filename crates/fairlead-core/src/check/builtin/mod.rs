pub mod agents;
pub mod controls;
pub mod exceptions;
pub mod generated;
pub mod leak;
pub mod notices;
pub mod paths;
pub mod refs;
pub mod scopes;
pub mod syntax;
pub mod tokens;
pub mod vendor;

use std::sync::Arc;

use crate::config::{ControlFile, KindDecl};
use crate::path::RepoPath;
use crate::registry::{FieldValue, Item, ItemId, Origin, Registry, ScopeId};

pub(crate) fn root_control(reg: &Registry) -> Option<&Arc<ControlFile>> {
    reg.scope(reg.root_scope())?.control.as_ref()
}

/// Nearest kind declaration up the chain of `from`.
pub(crate) fn kind_decl<'a>(reg: &'a Registry, from: &ScopeId, kind: &str) -> Option<&'a KindDecl> {
    reg.chain(from)
        .into_iter()
        .rev()
        .find_map(|s| reg.scope(s)?.control.as_ref()?.kinds.get(kind))
}

/// `kind.name` without the scope qualifier (what messages and `related` show).
pub(crate) fn bare_id(i: &ItemId) -> String {
    format!("{}.{}", i.kind.as_str(), i.name.as_str())
}

pub(crate) fn origin_loc(it: &Item) -> (RepoPath, u32, u32) {
    match &it.origin {
        Origin::DocDef { path, span } => (path.clone(), span.line, span.col),
        Origin::ControlTable { control } => (control.clone(), 0, 0),
    }
}

pub(crate) fn field_str<'a>(it: &'a Item, k: &str) -> Option<&'a str> {
    match it.fields.get(k)? {
        FieldValue::Scalar(s) => Some(s.as_str()),
        FieldValue::List(_) => None,
    }
}

pub(crate) fn field_list(it: &Item, k: &str) -> Vec<String> {
    match it.fields.get(k) {
        Some(FieldValue::List(v)) => v.clone(),
        Some(FieldValue::Scalar(s)) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        None => Vec::new(),
    }
}
