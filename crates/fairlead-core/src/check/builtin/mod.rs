pub mod agents;
pub mod authority;
pub mod bindings;
pub mod cascade;
pub mod controls;
pub mod coverage;
pub mod envs;
pub mod exceptions;
pub mod generated;
pub mod grave;
pub mod hier;
pub mod leak;
pub mod notices;
pub mod paths;
pub mod plans;
pub mod refs;
pub mod scopes;
pub mod syntax;
pub mod tokens;
pub mod variation;
pub mod vendor;
pub mod versions;

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
        Some(FieldValue::Scalar(s)) => s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

pub(crate) fn edition_of(reg: &Registry) -> crate::discover::Edition {
    crate::discover::Edition(
        root_control(reg)
            .and_then(|c| c.fairlead.edition.as_ref().map(|e| e.0))
            .unwrap_or(1),
    )
}
pub(crate) fn control_path(reg: &Registry, s: &ScopeId) -> Option<RepoPath> {
    reg.scope(s)?.control.as_ref().map(|c| c.path.clone())
}

/// The baseline file the root control configures, else `<folder>/<gen>/baseline.txt` (as `generate`).
pub(crate) fn baseline_rel(reg: &Registry) -> String {
    let ctl = root_control(reg);
    let folder = ctl
        .and_then(|c| c.settings.folder.clone())
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| "fairlead".to_string());
    let gen_dir = ctl
        .map(|c| c.layout.r#gen.clone())
        .filter(|g| !g.is_empty())
        .unwrap_or_else(|| "gen".to_string());
    ctl.map(|c| c.baseline.file.clone())
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| format!("{folder}/{gen_dir}/baseline.txt"))
}
