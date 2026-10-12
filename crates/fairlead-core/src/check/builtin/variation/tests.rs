//! Tests of the variation check, compiled under #[cfg(test)] from `variation.rs`.
use super::*;
use crate::check::testkit::{World, of};
use crate::diag::codes as c;

/// Root: a `ui` ladder, code kinds `cmp` and `ctr`, `[variation]` with `ui.atom` gated.
const ROOT: &str = "[layers.ui]\natom = 10\nmolecule = 20\n[kinds.cmp]\nshape = \"code\"\n[kinds.ctr]\nshape = \"code\"\n[kinds.axis]\nshape = \"code\"\n[variation]\nrequest-layers = [\"ui.atom\"]\n";
const WEB: &str = "[fairlead]\nscope = \"web\"\n";
const KIT: &str = "// fairlead: def ctr.kit\nexport const kit = 1;\n// fairlead: def cmp.button layer=ui.atom container=ctr.kit vis=public\nexport function Button() {}\n";
const REQ: &str = "d-7k3m9qxa";
const ANS: &str = "d-91cc04ab";

fn world() -> World {
    World::new()
        .root(ROOT)
        .file("src/kit/button.tsx", KIT)
        .scope("web", WEB)
}

fn request(extra: &str) -> String {
    format!(
        "+++\nkind = \"request\"\nid = \"{REQ}\"\nscope = \"web\"\nneed = \"root:cmp.button\"\nwant = \"density axis value compact\"\nbecause = \"dense data tables\"\nsum = \"Compact button\"\nout = \"\"\ncreated = \"0.1.0\"\n{extra}+++\n# Compact button\n"
    )
}

fn answer(status: &str, kind: &str, extra: &str) -> String {
    format!(
        "+++\nkind = \"change\"\nid = \"{ANS}\"\nstatus = \"{status}\"\nimpact = \"add\"\nscope = \"root\"\ncovers = [\"src/**\"]\ntype = \"{kind}\"\nanswers = [\"{REQ}\"]\ntouches = [\"cmp.button\"]\nresult = \"cmp.button-compact\"\naffects = \"all\"\n{extra}+++\n# Answer\n"
    )
}

fn approve(f: &mut crate::facts::Facts, kind: &str) {
    f.approvals.approvals.push(crate::config::Approval {
        fp: var::answer_fp(ANS, &[REQ.to_owned()], kind),
        target: Id::parse("var.answer").unwrap(),
        scope: fairlead_syntax::ids::ScopeSlug::parse("root").unwrap(),
        approver: "@platform".into(),
        date: "2026-10-09".parse().unwrap(),
        renewal: 0,
    });
}

const COMPACT: &str = "// fairlead: def cmp.button-compact layer=ui.atom container=ctr.kit variant-of=cmp.button native=compact\n// fairlead: use cmp.button\nexport function Compact() {}\n";

#[test]
fn r_var_001__a_second_declaration_of_a_unit_is_dup_id() {
    // A second `def` in code is another site of the same unit (it binds one more file); a second
    // declaration is a second registry doc declaring the ID in the same scope.
    let doc = "+++\ndirectives = ['def cmp.card layer=ui.atom']\n+++\n# Card\n";
    let b = world()
        .file("fairlead/contract/a.md", doc)
        .file("fairlead/contract/b.md", doc)
        .build();
    let codes: Vec<_> = b.registry.diagnostics().iter().map(|d| d.code).collect();
    assert!(codes.contains(&c::HN_D001), "one home per unit: {codes:?}");
}

#[test]
fn r_var_012__variant_of_only_in_the_base_scope() {
    let ds = world()
        .file("web/src/b.tsx", "// fairlead: def cmp.wide layer=ui.atom variant-of=root:cmp.button\n// fairlead: use root:cmp.button\nexport function W() {}\n")
        .build()
        .run_check(&VariationCheck);
    let d = of(&ds, c::HN_C013);
    assert_eq!(d.len(), 1, "{ds:#?}");
    assert!(d[0].hint.contains("new request"));
}

#[test]
fn r_var_050__a_variant_inherits_kind_layer_container_and_uses_its_base() {
    let ok = world()
        .file("src/kit/compact.tsx", COMPACT)
        .file(
            "fairlead/docs/changes/d-91cc04ab-a.md",
            &answer("draft", "variant", ""),
        )
        .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""))
        .build()
        .run_check(&VariationCheck);
    assert!(of(&ok, c::HN_S016).is_empty(), "{ok:#?}");
    let bad = world()
        .file("src/other.tsx", "// fairlead: def ctr.other\nexport const o = 1;\n// fairlead: def cmp.big layer=ui.molecule container=ctr.other variant-of=cmp.button request=x\nexport function Big() {}\n")
        .build()
        .run_check(&VariationCheck);
    let msgs: Vec<&str> = of(&bad, c::HN_S016)
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        msgs.iter().any(|m| m.contains("layer ui.molecule")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("neither its base's ctr.kit nor a sibling")),
        "{msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("has no `use cmp.button`")),
        "{msgs:?}"
    );
}

#[test]
fn r_var_020__a_request_layer_variant_needs_an_answer() {
    let bare = world()
        .file("src/kit/compact.tsx", COMPACT)
        .build()
        .run_check(&VariationCheck);
    let x = of(&bare, c::HN_X002);
    assert_eq!(x.len(), 1, "{bare:#?}");
    assert!(x[0].message.contains("goes through a request"));
    let answered = world()
        .file("src/kit/compact.tsx", COMPACT)
        .file(
            "fairlead/docs/changes/d-91cc04ab-a.md",
            &answer("draft", "variant", ""),
        )
        .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""))
        .build()
        .run_check(&VariationCheck);
    assert!(of(&answered, c::HN_X002).is_empty(), "{answered:#?}");
    // a unit outside the request layers needs no request
    let free = World::new()
        .root(ROOT)
        .file("src/kit/m.tsx", "// fairlead: def ctr.kit\nexport const k = 1;\n// fairlead: def cmp.card layer=ui.molecule container=ctr.kit\nexport function C() {}\n// fairlead: def cmp.card-wide layer=ui.molecule container=ctr.kit variant-of=cmp.card\n// fairlead: use cmp.card\nexport function W() {}\n")
        .build()
        .run_check(&VariationCheck);
    assert!(of(&free, c::HN_X002).is_empty(), "{free:#?}");
}

#[test]
fn r_var_021__a_wrapper_uses_what_it_wraps() {
    let ds = world()
        .file("web/src/t.tsx", "// fairlead: def cmp.table-button layer=ui.molecule wraps=root:cmp.button\nexport function T() {}\n")
        .build()
        .run_check(&VariationCheck);
    assert_eq!(of(&ds, c::HN_S016).len(), 1, "{ds:#?}");
    let ds = world()
        .file("web/src/t.tsx", "// fairlead: def cmp.table-button layer=ui.molecule wraps=root:cmp.button\n// fairlead: use root:cmp.button\nexport function T() {}\n")
        .build()
        .run_check(&VariationCheck);
    assert!(of(&ds, c::HN_S016).is_empty(), "{ds:#?}");
}

#[test]
fn r_var_022__a_consumer_concern_is_a_new_id_not_the_primitive() {
    let b = world()
        .file(
            "web/src/s.tsx",
            "// fairlead: def cmp.button layer=ui.atom\nexport function S() {}\n",
        )
        .build();
    assert!(
        b.registry
            .diagnostics()
            .iter()
            .any(|d| d.code == c::HN_R005),
        "reusing the ID shadows"
    );
    let b = world()
        .file("web/src/s.tsx", "// fairlead: def cmp.sidebar-button layer=ui.molecule extends=root:cmp.button\n// fairlead: use root:cmp.button\nexport function S() {}\n")
        .build();
    assert!(
        b.registry
            .diagnostics()
            .iter()
            .all(|d| d.code != c::HN_R005)
    );
}

#[test]
fn r_var_051__many_variants_suggest_an_axis() {
    let mut w = world().file(
        "fairlead/docs/changes/d-91cc04ab-a.md",
        &answer("draft", "variant", ""),
    );
    for n in ["a", "b", "c", "d"] {
        w = w.file(
            &format!("src/kit/{n}.tsx"),
            &format!("// fairlead: def cmp.button-{n} layer=ui.atom container=ctr.kit variant-of=cmp.button\n// fairlead: use cmp.button\nexport function V() {{}}\n"),
        );
    }
    let ds = w.build().run_check(&VariationCheck);
    let s = of(&ds, c::HN_W004);
    assert_eq!(s.len(), 1, "{ds:#?}");
    assert!(s[0].message.contains("cmp.button has 4 variants"));
    assert_eq!(s[0].severity, Severity::Info);
}

#[test]
fn r_var_053__native_spellings_are_unique_per_unit() {
    let twin = "// fairlead: def cmp.button-tight layer=ui.atom container=ctr.kit variant-of=cmp.button native=compact\n// fairlead: use cmp.button\nexport function T() {}\n";
    let ds = world()
        .file("src/kit/compact.tsx", COMPACT)
        .file("src/kit/tight.tsx", twin)
        .build()
        .run_check(&VariationCheck);
    let d: Vec<_> = of(&ds, c::HN_S016)
        .into_iter()
        .filter(|d| d.message.contains("native=compact"))
        .collect();
    assert_eq!(d.len(), 1, "{ds:#?}");
}

#[test]
fn r_var_070__an_adapter_names_its_rejection_or_wraps_an_ext() {
    let ds = world()
        .file("web/src/a.tsx", "// fairlead: def cmp.table-button layer=ui.molecule adapter=true\nexport function A() {}\n")
        .build()
        .run_check(&VariationCheck);
    assert!(
        of(&ds, c::HN_S016)
            .iter()
            .any(|d| d.message.contains("neither rejected-by= nor wraps=")),
        "{ds:#?}"
    );
    let ds = world()
        .file("fairlead/docs/changes/d-91cc04ab-a.md", &answer("draft", "variant", ""))
        .file("web/src/a.tsx", &format!("// fairlead: def cmp.table-button layer=ui.molecule adapter=true rejected-by={ANS}\nexport function A() {{}}\n"))
        .build()
        .run_check(&VariationCheck);
    assert!(
        of(&ds, c::HN_S016)
            .iter()
            .any(|d| d.message.contains("which is not a rejection")),
        "{ds:#?}"
    );
    let ds = world()
        .file("web/src/a.tsx", "// fairlead: def cmp.table-button layer=ui.molecule adapter=true rejected-by=d-zzzzzzzz\nexport function A() {}\n")
        .build()
        .run_check(&VariationCheck);
    assert_eq!(of(&ds, c::HN_R001).len(), 1, "{ds:#?}");
}

#[test]
fn r_var_072__an_ext_import_outside_the_adapter_is_a_bypass() {
    let ext = "+++\ndirectives = ['def ext.shadcn-button vendor=shadcn imports=\"@/components/ui/button\" path=\"src/components/ui/button.tsx\"']\n+++\n# Externals\n";
    let adapter = "// fairlead: def cmp.atom-button layer=ui.atom container=ctr.kit wraps=ext.shadcn-button adapter=true import-as=\"@/atoms/button\"\nimport { Button } from '@/components/ui/button';\nexport const A = Button;\n";
    let ds = world()
        .file("fairlead/contract/ext.md", ext)
        .file("src/kit/atom.tsx", adapter)
        .file("src/page.tsx", "import { Button } from '@/components/ui/button';\nimport { A } from '@/atoms/button';\n")
        .build()
        .run_check(&VariationCheck);
    let w = of(&ds, c::HN_W002);
    assert_eq!(w.len(), 1, "only outside the adapter: {ds:#?}");
    assert_eq!(
        (w[0].location.path.as_str(), w[0].location.line),
        ("src/page.tsx", 1)
    );
    assert!(w[0].hint.contains("import @/atoms/button instead"));
    assert_eq!(w[0].severity, Severity::Error);
}

#[test]
fn r_var_030__a_request_has_its_fields_and_a_declared_need() {
    let bad =
        "+++\nkind = \"request\"\nid = \"d-7k3m9qxa\"\nneed = \"cmp.nope\"\nwant = \"x\"\n+++\n";
    let ds = world()
        .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", bad)
        .build()
        .run_check(&VariationCheck);
    let s: Vec<&str> = of(&ds, c::HN_S004)
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(s.len(), 3, "because, sum, out: {s:?}");
    assert!(
        of(&ds, c::HN_R001)
            .iter()
            .any(|d| d.message.contains("needs cmp.nope")),
        "edge case 20"
    );
    let ok = world()
        .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""))
        .build()
        .run_check(&VariationCheck);
    assert!(
        of(&ok, c::HN_S004).is_empty() && of(&ok, c::HN_R001).is_empty(),
        "{ok:#?}"
    );
}

#[test]
fn r_var_031__status_derived_open_answered_closed() {
    let wrapper = format!(
        "// fairlead: def cmp.table-button layer=ui.molecule wraps=root:cmp.button\n// fairlead: use root:cmp.button\n// fairlead: flag awaiting to={REQ} target=9.0.0\nexport function T() {{}}\n"
    );
    let status = |files: &[(&str, String)], approved: bool| {
        let mut w = world().file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""));
        for (p, t) in files {
            w = w.file(p, t);
        }
        let b = w
            .facts(move |f| {
                if approved {
                    approve(f, "variant");
                }
            })
            .build();
        let (docs, _) = crate::docs::collect_change_docs(&b.corpus);
        let a = var::answers(&docs, &b.facts.approvals);
        let aw = var::awaiting(&b.registry, &b.corpus);
        var::status(&crate::docs::DraftId::parse(REQ).unwrap(), "web", &a, &aw)
    };
    let ans = (
        "fairlead/docs/changes/d-91cc04ab-a.md",
        answer("ready", "variant", ""),
    );
    let wrap = ("web/src/t.tsx", wrapper);
    assert_eq!(status(&[], false), ReqStatus::Open);
    assert_eq!(
        status(std::slice::from_ref(&ans), false),
        ReqStatus::Open,
        "an unapproved answer accepts nothing (R-VAR-043)"
    );
    assert_eq!(
        status(&[ans.clone(), wrap.clone()], true),
        ReqStatus::Answered,
        "a wrapper still awaits it"
    );
    assert_eq!(
        status(&[ans], true),
        ReqStatus::Closed,
        "migrated: no awaiting flag left"
    );
}

#[test]
fn r_var_032__stale_request_listed() {
    let mut w = world().file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""));
    for v in ["0.2.0", "0.3.0", "0.4.0", "0.5.0"] {
        w = w.file(&format!("fairlead/versions/{v}.md"), "+++\n+++\n");
    }
    assert!(
        of(&w.build().run_check(&VariationCheck), c::HN_W001).is_empty(),
        "4 seals is not more than 4"
    );
    let mut w = world().file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""));
    for v in ["0.2.0", "0.3.0", "0.4.0", "0.5.0", "0.6.0"] {
        w = w.file(&format!("fairlead/versions/{v}.md"), "+++\n+++\n");
    }
    let ds = w.build().run_check(&VariationCheck);
    let s = of(&ds, c::HN_W001);
    assert_eq!(s.len(), 1, "{ds:#?}");
    assert!(s[0].message.contains("open after 5 seals"));
}

#[test]
fn r_var_040__an_answer_names_requests_and_a_type() {
    let ds = world()
        .file(
            "fairlead/docs/changes/d-91cc04ab-a.md",
            &answer("draft", "maybe", ""),
        )
        .build()
        .run_check(&VariationCheck);
    assert!(
        of(&ds, c::HN_R001).iter().any(|d| d
            .message
            .contains("answers d-7k3m9qxa, which is no request")),
        "{ds:#?}"
    );
    assert!(
        of(&ds, c::HN_S004)
            .iter()
            .any(|d| d.message.contains("type is maybe")),
        "{ds:#?}"
    );
}

#[test]
fn r_var_041__affects_lists_every_using_scope() {
    let user = "// fairlead: use root:cmp.button\nexport const u = 1;\n";
    let doc =
        answer("draft", "variant", "").replace("affects = \"all\"\n", "affects = [\"api\"]\n");
    let ds = world()
        .file("web/src/u.tsx", user)
        .file("fairlead/docs/changes/d-91cc04ab-a.md", &doc)
        .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""))
        .build()
        .run_check(&VariationCheck);
    let s = of(&ds, c::HN_S004);
    assert_eq!(s.len(), 1, "{ds:#?}");
    assert!(s[0].message.contains("used from web"));
    assert!(s[0].hint.contains("affects = [\"web\"]"));
    let all = world()
        .file("web/src/u.tsx", user)
        .file(
            "fairlead/docs/changes/d-91cc04ab-a.md",
            &answer("draft", "variant", ""),
        )
        .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""))
        .build()
        .run_check(&VariationCheck);
    assert!(of(&all, c::HN_S004).is_empty(), "affects = \"all\"");
}

#[test]
fn r_var_042__a_rejection_leaves_a_graveyard_entry_and_a_repeat_is_caught() {
    let grave = "+++\nid = \"grv.compact-button\"\nstate = \"rejected\"\nwhat = \"compact button\"\nwhy = \"density belongs to the table\"\nmatch = [\"density axis\"]\n+++\n";
    let run = |with_grave: bool| {
        let mut w = world()
            .file(
                "fairlead/docs/changes/d-91cc04ab-a.md",
                &answer("ready", "rejected", ""),
            )
            .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""))
            .facts(|f| approve(f, "rejected"));
        if with_grave {
            w = w.file("fairlead/docs/graveyard/compact.md", grave);
        }
        w.build().run_check(&VariationCheck)
    };
    let without = run(false);
    assert!(
        of(&without, c::HN_S004)
            .iter()
            .any(|d| d.message.contains("no graveyard entry covers its want")),
        "{without:#?}"
    );
    let with = run(true);
    assert!(of(&with, c::HN_S004).is_empty(), "{with:#?}");
    assert_eq!(
        of(&with, c::HN_L002).len(),
        1,
        "the request itself now matches the graveyard (edge case 17)"
    );
}

#[test]
fn r_var_043__an_answer_needs_the_home_owner_approval() {
    let run = |approved: bool| {
        world()
            .file(
                "fairlead/docs/changes/d-91cc04ab-a.md",
                &answer("ready", "variant", ""),
            )
            .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""))
            .facts(move |f| {
                if approved {
                    approve(f, "variant");
                }
            })
            .build()
            .run_check(&VariationCheck)
    };
    let ds = run(false);
    let x: Vec<_> = of(&ds, c::HN_X002)
        .into_iter()
        .filter(|d| d.message.contains("not approved"))
        .collect();
    assert_eq!(x.len(), 1, "{ds:#?}");
    assert!(x[0].hint.contains(&format!("fairlead approve {ANS}")));
    assert!(
        of(&run(true), c::HN_X002)
            .iter()
            .all(|d| !d.message.contains("not approved"))
    );
}

fn wrapper(flag: &str) -> String {
    format!(
        "// fairlead: def cmp.table-button layer=ui.molecule wraps=root:cmp.button\n// fairlead: use root:cmp.button\n// fairlead: {flag}\nexport function T() {{}}\n"
    )
}

#[test]
fn r_var_060__awaiting_needs_to_and_target() {
    let ds = world()
        .file("web/src/t.tsx", &wrapper("flag awaiting"))
        .build()
        .run_check(&VariationCheck);
    assert!(
        of(&ds, c::HN_S016)
            .iter()
            .any(|d| d.message.contains("needs to=<request> and target=")),
        "{ds:#?}"
    );
    let ds = world()
        .file(
            "web/src/t.tsx",
            &wrapper("flag awaiting to=d-zzzzzzzz target=9.0.0"),
        )
        .build()
        .run_check(&VariationCheck);
    assert!(
        of(&ds, c::HN_R001)
            .iter()
            .any(|d| d.message.contains("d-zzzzzzzz, which is no request")),
        "{ds:#?}"
    );
}

#[test]
fn r_var_061__a_wrapper_ramps_by_version() {
    let at = |target: &str| {
        let mut w = world()
            .file(
                "web/fairlead/docs/requests/d-7k3m9qxa-b.md",
                &request("").replace("created = \"0.1.0\"", "created = \"0.4.0\""),
            )
            .file(
                "web/src/t.tsx",
                &wrapper(&format!("flag awaiting to={REQ} target={target}")),
            );
        w = w.file("fairlead/versions/0.4.0.md", "+++\n+++\n"); // sealed 0.4.0, open 0.4.1
        let ds = w.build().run_check(&VariationCheck);
        of(&ds, c::HN_X001).first().map(|d| d.severity)
    };
    assert_eq!(at("0.9.0"), None);
    assert_eq!(at("0.6.0"), Some(Severity::Info), "two minors ahead");
    assert_eq!(at("0.5.0"), Some(Severity::Warn), "due next");
    assert_eq!(
        at("0.4.1"),
        Some(Severity::Error),
        "open version reached the target"
    );
}

#[test]
fn r_var_062__awaiting_a_rejected_request_is_an_error() {
    let ds = world()
        .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""))
        .file("fairlead/docs/changes/d-91cc04ab-a.md", &answer("ready", "rejected", ""))
        .file("fairlead/docs/graveyard/compact.md", "+++\nid = \"grv.c\"\nstate = \"rejected\"\nwhat = \"c\"\nwhy = \"w\"\nmatch = [\"density axis\"]\n+++\n")
        .file("web/src/t.tsx", &wrapper(&format!("flag awaiting to={REQ} target=9.0.0")))
        .facts(|f| approve(f, "rejected"))
        .build()
        .run_check(&VariationCheck);
    let x = of(&ds, c::HN_X001);
    assert_eq!(x.len(), 1, "{ds:#?}");
    assert!(x[0].message.contains("which was rejected"));
    assert_eq!(x[0].severity, Severity::Error);
}

const FORK_ROOT: &str = "[variation]\nfork-detect = \"observe\"\n";
fn fork_world(root_extra: &str) -> World {
    World::new()
        .root(&format!("[layers.ui]\natom = 10\n[kinds.cmp]\nshape = \"code\"\n{root_extra}"))
        .scope("web", WEB)
        .scope("admin", "[fairlead]\nscope = \"admin\"\n")
        .file("src/Button.tsx", "// fairlead: def cmp.button layer=ui.atom sym=PrimaryButton\nexport function PrimaryButton() {}\n")
        .file("web/src/Button.tsx", "// fairlead: def cmp.web-button layer=ui.atom sym=PrimaryButton\nexport function PrimaryButton() {}\n")
}

#[test]
fn r_var_084__fork_detection_off_by_default() {
    let ds = fork_world("").build().run_check(&VariationCheck);
    assert!(of(&ds, c::HN_W003).is_empty(), "{ds:#?}");
    let ds = fork_world(FORK_ROOT).build().run_check(&VariationCheck);
    let w = of(&ds, c::HN_W003);
    assert!(!w.is_empty(), "{ds:#?}");
    assert!(
        w.iter()
            .all(|d| d.stage == Some(crate::config::Stage::Observe)),
        "starts at observe"
    );
}

#[test]
fn r_var_081__same_sym_or_basename_in_two_scopes_is_a_suspect() {
    let ds = fork_world(FORK_ROOT).build().run_check(&VariationCheck);
    let w = of(&ds, c::HN_W003);
    assert!(
        w.iter().any(|d| d.message.contains("sym PrimaryButton")),
        "{w:#?}"
    );
    assert!(
        w.iter().any(|d| d.message.contains("file name Button.tsx")),
        "{w:#?}"
    );
}

#[test]
fn r_var_080__a_declared_fork_is_an_exception() {
    let decl = "+++\ndirectives = ['ovr rule.var-fork of=cmp.web-button why=\"diverges on purpose\" owner=@web until=1.0.0']\n+++\n# Forks\n";
    let ds = fork_world(FORK_ROOT)
        .file("web/fairlead/forks.md", decl)
        .build()
        .run_check(&VariationCheck);
    assert!(of(&ds, c::HN_W003).is_empty(), "{ds:#?}");
}

#[test]
fn r_var_082__dismissed_pair_refires_on_third_location() {
    let locs = |scopes: &[&str]| -> BTreeSet<String> {
        scopes
            .iter()
            .map(|s| format!("{s}:PrimaryButton"))
            .collect()
    };
    let pair = pair_hash(&locs(&["root", "web"]));
    let dismiss = format!(
        "+++\ndirectives = ['ovr rule.var-fork of=cmp.web-button dismiss=true pair={pair} why=\"coincidence\"']\n+++\n# Forks\n"
    );
    let two = fork_world(FORK_ROOT)
        .file("web/fairlead/forks.md", &dismiss)
        .build()
        .run_check(&VariationCheck);
    assert!(
        of(&two, c::HN_W003)
            .iter()
            .all(|d| !d.message.contains("sym PrimaryButton")),
        "{two:#?}"
    );
    let three = fork_world(FORK_ROOT)
        .file("web/fairlead/forks.md", &dismiss)
        .file("admin/src/Primary.tsx", "// fairlead: def cmp.admin-button layer=ui.atom sym=PrimaryButton\nexport function PrimaryButton() {}\n")
        .build()
        .run_check(&VariationCheck);
    assert!(
        of(&three, c::HN_W003)
            .iter()
            .any(|d| d.message.contains("sym PrimaryButton")),
        "a third location re-flags: {three:#?}"
    );
}

#[test]
fn r_var_083__generated_and_vendor_excluded() {
    let gen_root =
        format!("{FORK_ROOT}[[vendor]]\nid = \"vendor.ui\"\npath = \"web/src/Button.tsx\"\n");
    let ds = fork_world(&gen_root).build().run_check(&VariationCheck);
    assert!(of(&ds, c::HN_W003).is_empty(), "{ds:#?}");
}

#[test]
fn r_var_063__wrappers_roll_up_to_the_ledger_and_sprawl_is_reported() {
    let mut w = world()
        .root("[exceptions]\nmax-live-per-rule = 1\n")
        .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", &request(""));
    for n in ["a", "b"] {
        w = w.file(
            &format!("web/src/{n}.tsx"),
            &format!("// fairlead: def cmp.wrap-{n} layer=ui.molecule wraps=root:cmp.button\n// fairlead: use root:cmp.button\n// fairlead: flag awaiting to={REQ} target=9.0.0\nexport function W() {{}}\n"),
        );
    }
    let b = w.build();
    let recs = var::awaiting_records(&b.registry, &b.corpus);
    assert_eq!(
        recs.iter()
            .map(|(p, l)| format!("{} {l}", p.as_str()))
            .collect::<Vec<_>>(),
        vec![
            format!("web/src/a.tsx cmp.wrap-a kind=awaiting to={REQ} target=9.0.0"),
            format!("web/src/b.tsx cmp.wrap-b kind=awaiting to={REQ} target=9.0.0"),
        ]
    );
    let ds = b.run_check(&VariationCheck);
    let s = of(&ds, c::HN_X011);
    assert_eq!(s.len(), 1, "{ds:#?}");
    assert!(s[0].message.contains("cmp.button has 2 interim wrappers"));
}

#[test]
fn r_ret_050__a_compacted_request_still_resolves() {
    let line =
        format!("{REQ} kind=request scope=web title=x closed=0.1.0 out=adopted sum=s git=- body=b");
    let ledger = format!(
        "# scope=web year=2026 prev=- edition=1\n{line} d={}\n",
        crate::hist::record_digest(&line)
    );
    let run = |files: &[(&str, String)]| {
        let l = ledger.clone();
        let mut w = world().facts(move |f| {
            f.ledgers.insert(
                crate::path::RepoPath::new("web/fairlead/ledger/hist/2026.hl").unwrap(),
                std::sync::Arc::from(l),
            );
        });
        for (p, t) in files {
            w = w.file(p, t);
        }
        w.build().run_check(&VariationCheck)
    };
    let ds = run(&[(
        "fairlead/docs/changes/d-91cc04ab-a.md",
        answer("draft", "variant", ""),
    )]);
    assert!(
        of(&ds, c::HN_R001).is_empty(),
        "the answer names a compacted request: {ds:#?}"
    );
    let ds = run(&[(
        "web/src/t.tsx",
        wrapper(&format!("flag awaiting to={REQ} target=9.0.0")),
    )]);
    let x = of(&ds, c::HN_X001);
    assert!(
        x.iter().any(|d| d.message.contains("closed and compacted")),
        "{ds:#?}"
    );
}
