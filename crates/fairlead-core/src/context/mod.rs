//! `fairlead context`: the pure brief builder (seam S9). No I/O, no clock, no writes besides the
//! disposable token-estimate cache (R-CTX-006).
pub mod brief;
pub mod budget;
pub mod digest;
pub mod estimate;
pub mod priority;
pub mod request;
pub use request::Since;
pub mod tiers;

pub use brief::{ContextBrief, ContextOutput, ContextStatus};
pub use digest::effective_digest;
pub use estimate::{TokenEstimate, estimate_bytes};
pub use priority::Priority;
pub use request::{
    Budget, ContextArgError, ContextInput, ContextRequest, ContextTarget, NO_MATCH_MESSAGE,
    missing_note, parse_target,
};
pub use tiers::Tier;

use crate::check::RuleSet;
use crate::config::Stage;
use crate::context::brief::{Block, BriefFooter, BriefHeader, ChainLink, Entry, Omission};
use crate::context::brief::{CAP_TEXT, CAP_TITLE, fence};
use crate::context::budget::{merge_omissions, pack};
use crate::context::estimate::estimate_text;
use crate::context::priority::Candidate;
use crate::context::priority::{OvrView, RuleView};
use crate::extract::Corpus;
use crate::hash::ContentHash;
use crate::hash::Digest;
use crate::path::RepoPath;
use crate::registry::OvrState;
use crate::registry::{Lookup, Registry, ScopeId};
use crate::render::sanitize::sanitize_data;

/// `usize` to `u32` without a lossy cast.
pub(crate) fn n32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Counts and flags the frame needs from the gatherers.
#[derive(Default)]
pub(crate) struct Shape {
    pub kids: u32,
    pub sibs: u32,
    pub ignored: bool,
}

/// One resolved view of the request: which scope, which chain, which digests.
pub(crate) struct Scene<'s> {
    pub input: &'s ContextInput<'s>,
    pub scope: ScopeId,
    pub chain: Vec<ScopeId>,
    pub digests: Vec<(ScopeId, Digest)>,
    pub generator: String,
    pub path: Option<RepoPath>,
    pub rules: RuleSet,
}

pub(crate) fn resolve_scope(reg: &Registry, req: &ContextRequest) -> (Option<RepoPath>, ScopeId) {
    let root = reg.root_scope().clone();
    let (path, scope) = match &req.target {
        ContextTarget::SessionStart => (None, root),
        ContextTarget::Path(p) => (Some(p.clone()), reg.scope_of(p).cloned().unwrap_or(root)),
        ContextTarget::IdLookup(id) => match reg.lookup(&root, id) {
            Lookup::Found(item) => (None, item.scope),
            _ => (None, root),
        },
    };
    let scope = match &req.scope_pin {
        Some(p) if reg.scope(p).is_some() => p.clone(),
        _ => scope,
    };
    (path, scope)
}

impl<'s> Scene<'s> {
    pub(crate) fn new(input: &'s ContextInput<'s>) -> Scene<'s> {
        let reg = input.registry;
        let (path, scope) = resolve_scope(reg, input.request);
        let chain: Vec<ScopeId> = reg.chain(&scope).into_iter().cloned().collect();
        let generator = digest::generator_version(input.report.header.edition);
        let digests = digest::chain_digests(reg, &scope, &generator);
        Scene {
            input,
            scope,
            chain,
            digests,
            generator,
            path,
            rules: RuleSet::collect(reg),
        }
    }

    pub(crate) fn slug(id: &ScopeId) -> String {
        id.0.as_str().to_string()
    }

    /// Hops from the brief's scope to `id` along the chain (R-CTX-042 proximity); off-chain scopes count as the chain length.
    pub(crate) fn dist(&self, id: &ScopeId) -> u32 {
        match self.chain.iter().position(|c| c == id) {
            Some(p) => n32(self.chain.len().saturating_sub(1).saturating_sub(p)),
            None => n32(self.chain.len()),
        }
    }

    /// Ancestors root first, without the brief's own scope.
    pub(crate) fn ancestors(&self) -> Vec<ChainLink> {
        let keep = self.digests.len().saturating_sub(1);
        self.digests
            .iter()
            .take(keep)
            .map(|(s, d)| ChainLink {
                scope: Self::slug(s),
                digest: d.as_str().to_string(),
            })
            .collect()
    }

    pub(crate) fn header(&self, shape: &Shape) -> BriefHeader {
        BriefHeader {
            scope: Self::slug(&self.scope),
            digest: self
                .digests
                .last()
                .map(|(_, d)| d.as_str().to_string())
                .unwrap_or_default(),
            edition: self.input.report.header.edition.0,
            chain: self.ancestors(),
            kids: shape.kids,
            sibs: shape.sibs,
            digest_only: self.input.request.digest_only,
            ignored: shape.ignored,
            compact: false,
            since: match &self.input.request.since {
                None => None,
                Some(Since::Changed { label, .. } | Since::Same { label }) => Some(label.clone()),
                Some(Since::Unknown) => Some("unknown".into()),
            },
        }
    }

    /// A valid `explain` argument for scope-level `+N more` lines (DD-7): the scope directory, else the `kinds` topic.
    pub(crate) fn explain_target(&self) -> String {
        self.input
            .registry
            .scope(&self.scope)
            .map(|s| s.dir.as_str().to_string())
            .filter(|d| !d.is_empty() && d != ".")
            .unwrap_or_else(|| "kinds".to_string())
    }

    /// Provenance hash of a file: its extraction record, else a control file of the registry, else zeros.
    pub(crate) fn fence_for_path(&self, path: &RepoPath, text: &str, cap: usize) -> String {
        let reg = self.input.registry;
        let hash = self
            .input
            .corpus
            .records
            .get(path)
            .map(|r| r.hash)
            .or_else(|| {
                reg.scopes().find_map(|s| {
                    s.control
                        .as_ref()
                        .filter(|c| c.path == *path)
                        .map(|c| c.content_hash)
                })
            })
            .unwrap_or(ContentHash([0; 32]));
        fence(path, &hash, text, cap)
    }

    /// The deepest effective override of `rule` declared in the chain (R-CTX-021).
    fn effective_ovr(&self, rule: &crate::check::DeclaredRule) -> Option<OvrView> {
        let reg = self.input.registry;
        let mut best: Option<(usize, &crate::registry::Override)> = None;
        for o in reg.overrides() {
            if o.state != OvrState::Effective
                || o.target.scope != rule.home
                || o.target.name != rule.id.0
                || !o.target.to_id().kind_name().starts_with("rule.")
            {
                continue;
            }
            let Some(pos) = self.chain.iter().position(|c| *c == o.declared_in) else {
                continue;
            };
            if best.is_none_or(|(b, _)| pos >= b) {
                best = Some((pos, o));
            }
        }
        best.map(|(_, o)| OvrView {
            scope: Self::slug(&o.declared_in),
            note: o
                .why
                .as_ref()
                .map(|t| self.fence_for_path(&o.at.path, t, CAP_TEXT)),
        })
    }

    pub(crate) fn rule_view(&self, r: &crate::check::DeclaredRule) -> RuleView {
        let reg = self.input.registry;
        let why = r.why.as_ref().and_then(|t| {
            let c = reg.scope(&r.home)?.control.as_ref()?;
            Some(fence(&c.path, &c.content_hash, t, CAP_TITLE))
        });
        RuleView {
            id: format!("rule.{}", r.id.0),
            home: Self::slug(&r.home),
            distance: self.dist(&r.home),
            sev: r.sev,
            stage: stage_name(r.stage).to_string(),
            glob: r.glob.iter().map(|g| sanitize_data(g, 80)).collect(),
            why,
            ovr: self.effective_ovr(r),
        }
    }

    /// Effective digest of any scope: the chain's own, else derived from its parent (siblings, children, descendants).
    pub(crate) fn digest_obj(&self, id: &ScopeId) -> Digest {
        if let Some((_, d)) = self.digests.iter().find(|(s, _)| s == id) {
            return d.clone();
        }
        let reg = self.input.registry;
        let parent = reg
            .scope(id)
            .and_then(|s| s.parent.as_ref())
            .map(|p| self.digest_obj(p));
        digest::effective_digest(
            parent.as_ref(),
            &reg.view(id).authored_digest,
            &self.generator,
        )
    }

    pub(crate) fn digest_of(&self, id: &ScopeId) -> String {
        self.digest_obj(id).as_str().to_string()
    }
}

pub(crate) fn stage_name(s: Stage) -> &'static str {
    match s {
        Stage::Observe => "observe",
        Stage::Warn => "warn",
        Stage::Enforce => "enforce",
    }
}

/// R-CTX-003: an existing path with no record at or under it is outside the scanned tree (ignored area).
fn path_is_ignored(corpus: &Corpus, p: &RepoPath) -> bool {
    !corpus
        .records
        .range(p.clone()..)
        .take_while(|(k, _)| k.as_str().starts_with(p.as_str()))
        .any(|(k, _)| k == p || k.is_within(p))
}

/// Candidate gatherers in rule order.
fn gather(
    scene: &Scene<'_>,
    cands: &mut Vec<Candidate>,
    pre: &mut Vec<Omission>,
    shape: &mut Shape,
) {
    shape.ignored = scene
        .path
        .as_ref()
        .is_some_and(|p| path_is_ignored(scene.input.corpus, p));
    priority::gather_rules(scene, cands);
    priority::gather_items(scene, cands, pre);
    priority::gather_drift(scene, cands);
    tiers::gather_scope_card(scene, cands);
    tiers::gather_neighbours(scene, cands, pre, shape);
    tiers::gather_ancestors(scene, cands);
    tiers::gather_graveyard(scene, cands);
    tiers::gather_open_work(scene, cands);
    if matches!(scene.input.request.target, ContextTarget::SessionStart) {
        tiers::gather_handoff(scene, cands);
    }
}

/// Pure (R-ARCH-050 family): the whole brief for one request.
pub fn build(input: &ContextInput<'_>) -> ContextOutput {
    let scene = Scene::new(input);
    let req = input.request;
    let bpt = input.config.bytes_per_token;
    let cap = req.budget.resolve(&req.target, input.config);

    let mut cands: Vec<Candidate> = Vec::new();
    let mut pre: Vec<Omission> = Vec::new();
    let mut shape = Shape::default();
    let cost = |e: &Entry| -> u32 {
        estimate_text(input.cache, &format!("{}\n", e.standalone()), bpt).tokens
    };
    if !req.digest_only {
        gather(&scene, &mut cands, &mut pre, &mut shape);
        // R-CTX-023: with --since, unchanged objects are elided; drift is the current state and
        // always stays. An unknown digest keeps the full slice (R-CTX-053).
        match &req.since {
            Some(Since::Changed { ids, .. }) => cands.retain(|c| {
                c.block == Block::Drift
                    || ids
                        .iter()
                        .any(|i| c.explain == *i || c.key.contains(i.as_str()))
            }),
            Some(Since::Same { label }) => {
                let n = cands.iter().filter(|c| c.block != Block::Drift).count();
                cands.retain(|c| c.block == Block::Drift);
                if n > 0 {
                    cands.push(Candidate {
                        priority: priority::Priority::P5Recent,
                        score: priority::Score::new(1, priority::Relevance::ScopeWide, 0),
                        hint: 0,
                        key: "9since".into(),
                        explain: "kinds".into(),
                        block: Block::Items,
                        entry: Entry::Note {
                            text: format!("~ {n} objects unchanged since {label}"),
                        },
                    });
                }
            }
            _ => {}
        }
        if matches!(req.target, ContextTarget::SessionStart) {
            let (card, rest): (Vec<Candidate>, Vec<Candidate>) =
                cands.into_iter().partition(|c| c.entry.tag() != "handoff");
            let (kept, dropped) =
                tiers::cap_l1(tiers::to_l1_card(card), &cost, tiers::L1_CAP_TOKENS);
            pre.extend(merge_omissions(&[], &dropped));
            cands = kept;
            cands.extend(rest);
        }
    }

    let frame_header = scene.header(&shape);
    let frame_footer = BriefFooter {
        digest: "00000000".to_string(),
        est: 0,
        next: "fairlead check".to_string(),
    };
    let measure = |kept: &[Entry], om: &[Omission]| -> u32 {
        let mut b = ContextBrief::assemble(
            frame_header.clone(),
            frame_footer.clone(),
            kept.to_vec(),
            om.to_vec(),
            None,
        );
        b.footer.next = b.next_hint();
        b.finalize(bpt).tokens
    };
    let base = measure(&[], &[]);
    let packed = pack(cands, pre, cap, base, &cost, &measure);

    let (header, status_line, status) = match packed.status {
        ContextStatus::Ok => (frame_header, None, ContextStatus::Ok),
        ContextStatus::TruncatedMandatory { min_budget } => (
            BriefHeader {
                compact: true,
                ..frame_header
            },
            Some(format!("truncated-mandatory: minimum budget {min_budget}")),
            packed.status,
        ),
    };
    let mut brief = ContextBrief::assemble(
        header,
        frame_footer,
        packed.kept,
        packed.omitted,
        status_line,
    );
    brief.footer.next = brief.next_hint();
    let tokens = brief.finalize(bpt);
    ContextOutput {
        brief,
        tokens,
        status,
    }
}

use crate::config::ConfigFailure;
use crate::config::ContextConfig;
use crate::error::Error;
use crate::registry::ScopeState;

/// R-CTX-007, R-CTX-043, R-CLI-032: everything that must exit 2 before a brief is built.
pub fn precheck(
    registry: &Registry,
    config: &ContextConfig,
    request: &ContextRequest,
) -> Result<(), Error> {
    if request.exact_tokens {
        estimate::exact_tokens("", config.tokenizer.as_deref())?;
    }
    if let Some(pin) = &request.scope_pin
        && registry.scope(pin).is_none()
    {
        return Err(Error::Usage(format!(
            "--scope: unknown scope \"{}\"",
            crate::render::sanitize::sanitize_data(pin.0.as_str(), 40)
        )));
    }
    let (_, scope) = resolve_scope(registry, request);
    for id in registry.chain(&scope) {
        let Some(s) = registry.scope(id) else {
            continue;
        };
        match &s.state {
            ScopeState::Resolved => {}
            ScopeState::Unresolved { reason } => {
                let path = s
                    .control
                    .as_ref()
                    .map(|c| c.path.clone())
                    .unwrap_or_else(|| s.dir.clone());
                return Err(Error::Config(Box::new(ConfigFailure {
                    path,
                    diags: reason.clone(),
                })));
            }
            ScopeState::AbsentFromTree => {
                return Err(Error::Usage(format!(
                    "scope \"{}\" is absent from the working tree (sparse checkout)",
                    Scene::slug(id)
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod testutil {
    use crate::config::{ContextConfig, SevWeight};

    pub fn cfg() -> ContextConfig {
        ContextConfig {
            budget_start: 500,
            budget_path: 700,
            bytes_per_token: 4,
            tokenizer: None,
            sev_weight: SevWeight {
                error: 4,
                warn: 2,
                info: 1,
            },
        }
    }

    use std::sync::Arc;

    use fairlead_syntax::diag::Span;
    use fairlead_syntax::directive::{Carrier, Directive, Extent, Verb};
    use fairlead_syntax::extract::{Extraction, ScanStatus};
    use fairlead_syntax::ids::Id;

    use crate::classify::FileClass;
    use crate::extract::{Corpus, FileRecord};
    use crate::hash::ContentHash;
    use crate::path::RepoPath;

    pub fn directive(verb: Verb, ids: &[&str]) -> Directive {
        Directive {
            ordinal: 0,
            verb,
            must_understand: false,
            ids: ids.iter().map(|s| Id::parse(s).unwrap()).collect(),
            bare: vec![],
            args: vec![],
            note: None,
            carrier: Carrier::LineComment,
            span: Span {
                start: 0,
                end: 0,
                line: 1,
                col: 1,
            },
            extent: Extent::File,
            end_span: None,
        }
    }

    pub fn rec(path: &str, directives: Vec<Directive>) -> FileRecord {
        FileRecord {
            path: RepoPath::new(path).unwrap(),
            class: FileClass::Authored,
            size: 1,
            hash: ContentHash([1; 32]),
            extraction: Some(Arc::new(Extraction {
                status: ScanStatus::Scanned,
                candidates: crate::context::n32(directives.len()),
                dropped: 0,
                directives,
                frontmatter: None,
                diags: vec![],
            })),
            css_tokens: vec![],
            literals: vec![],
            secrets: vec![],
            pattern_hits: vec![],
            env_tokens: vec![],
            grave: None,
            css: None,
            idents: vec![],
            imports: vec![],
            skip: None,
        }
    }

    pub fn corpus(recs: Vec<FileRecord>) -> Corpus {
        Corpus {
            records: recs.into_iter().map(|r| (r.path.clone(), r)).collect(),
        }
    }

    use fairlead_syntax::diag::{DiagCode, Severity};

    use crate::check::fingerprint::{FingerprintInput, fingerprint};
    use crate::check::{Finding, FindingState, RuleId};
    use crate::config::Stage;
    use crate::diag::Location;
    use crate::registry::ScopeId;
    use fairlead_syntax::ids::ScopeSlug;

    pub fn finding(
        code: DiagCode,
        sev: Severity,
        path: &str,
        line: u32,
        state: FindingState,
        rule: Option<&str>,
    ) -> Finding {
        let scope = ScopeId(ScopeSlug::parse("root").unwrap());
        let rp = RepoPath::new(path).unwrap();
        let rid = rule.map(|r| RuleId(Id::parse(&format!("rule.{r}")).unwrap().name));
        let fp = fingerprint(&FingerprintInput {
            code,
            rule: rid.as_ref(),
            scope: &scope,
            path: &rp,
            text: "x",
            occurrence: line,
        });
        Finding {
            code,
            severity: sev,
            stage: Stage::Enforce,
            location: Location {
                path: rp,
                line,
                col: 1,
            },
            scope,
            rule: rid,
            related: None,
            message: "m".into(),
            excerpt: None,
            intent: None,
            fix: "f".into(),
            occurrence: line,
            fingerprint: fp,
            state,
            waived_by: None,
        }
    }

    use crate::check::{CheckReport, ReportHeader, Summary};
    use crate::discover::Edition;
    use crate::time::AsOf;

    pub fn report(findings: Vec<Finding>, partial: bool) -> CheckReport {
        CheckReport {
            header: ReportHeader {
                fairlead_version: "0.1.0".into(),
                edition: Edition(1),
                as_of: AsOf::parse("2026-10-09").unwrap(),
                scopes: 1,
                files: 1,
                rules: 1,
            },
            findings,
            summary: Summary::default(),
            partial,
            unknown_scopes: vec![],
            absent_scopes: vec![],
            zero_declarations: false,
        }
    }
}

#[cfg(test)]
mod guard {
    #[test]
    fn r_ctx_006__context_sources_never_touch_the_filesystem_clock_env_or_processes() {
        let sources = [
            ("mod.rs", include_str!("mod.rs")),
            ("request.rs", include_str!("request.rs")),
            ("estimate.rs", include_str!("estimate.rs")),
            ("digest.rs", include_str!("digest.rs")),
            ("brief.rs", include_str!("brief.rs")),
            ("priority.rs", include_str!("priority.rs")),
            ("budget.rs", include_str!("budget.rs")),
            ("tiers.rs", include_str!("tiers.rs")),
        ];
        let forbidden = [
            concat!("std::", "fs"),
            concat!("File", "::open"),
            concat!("Command", "::new"),
            concat!("thread", "::spawn"),
            concat!("std::", "process"),
            concat!("std::", "env"),
            concat!("SystemTime", "::now"),
            concat!("Instant", "::now"),
            concat!("Hash", "Map"),
            concat!("Hash", "Set"),
        ];
        for (name, src) in sources {
            for needle in forbidden {
                assert!(!src.contains(needle), "{name} contains {needle}");
            }
        }
    }
}
