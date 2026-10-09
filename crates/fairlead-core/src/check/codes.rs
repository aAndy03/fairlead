//! The diagnostic registry: one row per code of chapter 13, all milestones (R-ARCH-055, R-CHK-042).
use std::sync::LazyLock;
use fairlead_syntax::diag::{DiagCode, Severity};
use crate::{check::floor::is_floor, config::Stage, diag::codes as c};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Milestone { M01, M02, M03 }
pub const ACTIVE_MILESTONE: Milestone = Milestone::M01;
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Emitter { Syntax, Config, Registry, Check, Generate }

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiagnosticSpec {
    pub code: DiagCode, pub mnemonic: &'static str, pub title: &'static str,
    pub default_severity: Severity, pub default_stage: Stage,
    pub floor: bool, pub waivable: bool, pub milestone: Milestone, pub emitter: Emitter,
}

fn rank(m: Milestone) -> u8 { match m { Milestone::M01 => 1, Milestone::M02 => 2, Milestone::M03 => 3 } }

macro_rules! row {
    ($code:ident, $mn:expr, $title:expr, $sev:ident, $stage:ident, $ms:ident, $em:ident) => {
        DiagnosticSpec { code: c::$code, mnemonic: $mn, title: $title, default_severity: Severity::$sev,
            default_stage: Stage::$stage, floor: false, waivable: false, milestone: Milestone::$ms, emitter: Emitter::$em }
    };
}
/// Later-milestone rows: title = mnemonic, emitter Check.
macro_rules! late { ($code:ident, $mn:expr, $sev:ident, $stage:ident, $ms:ident) => { row!($code, $mn, $mn, $sev, $stage, $ms, Check) }; }

/// Only these may be waived (chapter 13 "Waivable" column); a waiver also needs a `waivable` rule item.
const WAIVABLE: [DiagCode; 5] = [c::HN_R002, c::HN_T001, c::HN_U001, c::HN_A001, c::HN_A002];

static TABLE: LazyLock<Vec<DiagnosticSpec>> = LazyLock::new(|| {
    let mut v = vec![
        // syntax (12) -- emitted by P2 / P4
        row!(HN_S001, "UNKNOWN-VERB", "unknown verb", Error, Enforce, M01, Syntax),
        row!(HN_S002, "UNKNOWN-KIND", "unknown kind", Error, Enforce, M01, Registry),
        row!(HN_S004, "FRONTMATTER", "frontmatter or directives array invalid", Error, Enforce, M01, Syntax),
        row!(HN_S005, "MARKER-NEARMISS", "near-miss marker", Warn, Warn, M01, Syntax),
        row!(HN_S006, "CONTINUATION", "continuation limit exceeded", Error, Enforce, M01, Syntax),
        row!(HN_S007, "BAD-QUOTE", "unterminated quote or unknown escape", Error, Enforce, M01, Syntax),
        row!(HN_S008, "UNMATCHED-END", "unmatched end or unclosed region", Error, Enforce, M01, Syntax),
        row!(HN_S009, "OVR-NO-FIELD", "ovr of a field the item lacks", Error, Enforce, M01, Registry),
        row!(HN_S010, "BINDS-NOTHING", "directive binds to nothing", Warn, Warn, M01, Syntax),
        row!(HN_S011, "UNKNOWN-KEY", "unknown directive key", Warn, Warn, M01, Registry),
        row!(HN_S012, "VERSION-EXPIRY", "version expiry before 0.2", Error, Enforce, M01, Syntax),
        row!(HN_S013, "RUSTDOC-LEAK", "marker in a doc comment", Error, Enforce, M01, Syntax),
        row!(HN_S014, "SYM-ABSENT", "sym not found in extent", Warn, Warn, M01, Syntax),
        row!(HN_S015, "TOO-MANY-DIRECTIVES", "too many directives in one file", Warn, Warn, M01, Syntax),
        row!(HN_S016, "OVR-INCOMPLETE", "incomplete ovr", Error, Enforce, M01, Registry),
        // notices
        row!(HN_I001, "FILE-SKIPPED", "file skipped", Info, Warn, M01, Check),
        row!(HN_I002, "COMMENT-SYNTAX", "unknown comment syntax", Info, Warn, M01, Syntax),
        row!(HN_I003, "ENCODING-SKIP", "non-UTF-8 file skipped", Warn, Warn, M01, Check),
        row!(HN_N001, "NOTHING-VERIFIED", "zero declarations", Info, Warn, M01, Check),
        row!(HN_N002, "SCOPE-ELIGIBLE", "scope eligible for promotion", Info, Observe, M01, Check),
        // references and IDs
        row!(HN_R001, "DANGLING", "dangling reference", Error, Enforce, M01, Check),
        row!(HN_R002, "UNUSED", "unused declaration", Warn, Observe, M01, Check),
        row!(HN_R005, "SHADOW", "shadowing without ovr", Error, Enforce, M01, Registry),
        row!(HN_R007, "AMBIG", "ambiguous unqualified reference", Error, Enforce, M01, Registry),
        row!(HN_D001, "DUP-ID", "duplicate ID", Error, Enforce, M01, Registry),
        row!(HN_D002, "TOO-MANY-SITES", "too many sites", Error, Enforce, M01, Check),
        // tokens and rules
        row!(HN_T001, "LEAK", "literal token leak", Error, Observe, M01, Check),
        row!(HN_T002, "OVR-MISSING", "value differs from contract without ovr", Error, Enforce, M01, Check),
        row!(HN_T003, "TOK-MISSING", "contract token missing from CSS", Error, Enforce, M01, Check),
        row!(HN_T004, "TOK-UNDECLARED", "undeclared CSS token", Error, Enforce, M01, Check),
        row!(HN_T005, "TOK-DERIVED", "derived token not re-declared", Error, Enforce, M01, Check),
        row!(HN_U001, "PATTERN", "pattern-rule finding", Warn, Observe, M01, Check),
        // generated and vendor
        row!(HN_G001, "STALE-GEN", "stale generated file", Error, Warn, M01, Check),
        row!(HN_G002, "HAND-EDITED", "hand-edited generated file", Error, Enforce, M01, Check),
        row!(HN_G004, "VENDOR-CHANGED", "vendor-owned file changed", Warn, Warn, M01, Check),
        row!(HN_G005, "VENDOR-EDIT", "edit to a vendor-owned file", Error, Enforce, M01, Check),
        // exceptions, baseline, authority
        row!(HN_X001, "EXC-EXPIRED", "expired exception", Error, Enforce, M01, Check),
        row!(HN_X002, "EXC-UNAPPROVED", "unapproved exception", Error, Warn, M01, Check),
        row!(HN_X003, "EXC-FLOOR", "exception on floor rule", Error, Enforce, M01, Check),
        row!(HN_X004, "EXC-UNUSED", "stale exception", Warn, Warn, M01, Check),
        row!(HN_X005, "EXC-INERT", "inert exception", Warn, Warn, M01, Check),
        row!(HN_X006, "EXC-HORIZON", "exception horizon", Error, Enforce, M01, Check),
        row!(HN_X007, "WAIVER-BUDGET", "waiver budget", Warn, Warn, M01, Check),
        row!(HN_X008, "WAIVER-RATE", "rule waiver rate", Info, Warn, M01, Check),
        row!(HN_X009, "RAMP-STALLED", "ramp stalled", Info, Warn, M01, Check),
        row!(HN_X010, "EXC-STALE", "exception target changed or vanished", Warn, Warn, M01, Check),
        row!(HN_X011, "EXC-SPRAWL", "exception sprawl", Warn, Warn, M01, Check),
        row!(HN_X012, "OWNER-UNKNOWN", "unknown owner on ovr", Error, Enforce, M01, Registry),
        row!(HN_X014, "DUP-OVR", "overlapping exceptions", Error, Enforce, M01, Registry),
        row!(HN_B001, "BASELINE-FIXED", "baseline entry fixed", Info, Warn, M01, Check),
        row!(HN_B002, "BASELINE-TAMPERED", "baseline tampered", Error, Enforce, M01, Generate),
        row!(HN_A001, "AGENT-BLOCK", "agent block missing or stale", Error, Warn, M01, Check),
        row!(HN_A002, "CODEOWNERS-GAP", "CODEOWNERS gap", Error, Warn, M01, Check),
        row!(HN_A004, "OWNER-UNCOVERED", "owner not covered by CODEOWNERS", Warn, Warn, M01, Check),
        // control files, scopes, floor
        row!(HN_C001, "CTL-PARSE", "unparseable control file", Error, Enforce, M01, Config),
        row!(HN_C002, "CTL-UNKNOWN-KEY", "unknown control key", Warn, Warn, M01, Config),
        row!(HN_C003, "CTL-DELEG", "non-delegable key", Error, Enforce, M01, Config),
        row!(HN_C004, "ROOT-DUP", "root problem", Error, Enforce, M01, Config),
        row!(HN_C005, "FOLDER-MISMATCH", "folder mismatch", Error, Enforce, M01, Config),
        row!(HN_C006, "PIN-MISMATCH", "pin mismatch", Error, Enforce, M01, Config),
        row!(HN_C007, "DUP-SCOPE-ID", "duplicate scope id", Error, Enforce, M01, Registry),
        row!(HN_C008, "SCOPE-UNREGISTERED", "managerial folder not registered", Error, Warn, M01, Registry),
        row!(HN_C009, "SCOPE-VANISHED", "registered scope folder missing", Error, Enforce, M01, Check),
        row!(HN_C011, "CLASS-LINE", "line class not allowed in this file class", Error, Enforce, M01, Check),
        row!(HN_C012, "EXT-CYCLE", "file extends cycle", Error, Enforce, M01, Check),
        row!(HN_C013, "HOME-MISMATCH", "home does not contain every site", Error, Enforce, M01, Check),
        row!(HN_F001, "SECRET", "secret-like value", Error, Enforce, M01, Check),
        row!(HN_P001, "PATH-HYGIENE", "path hygiene", Error, Enforce, M01, Config),
        row!(HN_V013, "H-GIT-SPARSE", "scope absent from sparse checkout", Warn, Warn, M01, Check),
        row!(HN_M001, "GLOB-DEAD", "glob matches nothing", Warn, Warn, M01, Check),
        row!(HN_M002, "SCOPE-REPARENTED", "scope reparented", Error, Enforce, M01, Check),
        row!(HN_M003, "SCOPE-ID-CHANGED", "scope id changed", Error, Enforce, M01, Check),
        // ---- later milestones: registered, inactive in 0.1 ----
        late!(HN_R004, "ALIAS-REF", Warn, Warn, M02), late!(HN_R006, "DEPRECATED-REF", Warn, Warn, M02),
        late!(HN_R008, "NOT-PROVIDED", Error, Enforce, M02), late!(HN_D003, "ID-RESERVED", Error, Enforce, M02),
        late!(HN_D004, "doc.undocumented-unit", Warn, Warn, M03), late!(HN_D005, "doc.dangling", Warn, Warn, M03),
        late!(HN_D006, "doc.uncovered", Warn, Warn, M02),
        late!(HN_T006, "var.token-near", Info, Warn, M03), late!(HN_T007, "var.token-unresolved", Info, Observe, M03),
        late!(HN_T008, "css.layer-order", Error, Enforce, M02), late!(HN_T009, "css.bare-override", Error, Enforce, M02),
        late!(HN_T010, "TOK-DERIVED-GRAPH", Error, Enforce, M02),
        late!(HN_G003, "LEDGER-CHECKPOINT", Error, Enforce, M02), late!(HN_G006, "FOREIGN-LOST", Warn, Warn, M02),
        late!(HN_X013, "APPROVER-AGENT", Error, Enforce, M02), late!(HN_X015, "H-AUTH-UNSIGNED", Warn, Warn, M02),
        late!(HN_X016, "EXC-CAP", Error, Enforce, M02), late!(HN_A003, "INSTRUCTION-TEXT", Info, Observe, M02),
        late!(HN_C014, "SCOPE-AMBIG", Error, Enforce, M02), late!(HN_K001, "SUMMARY-MISSING", Error, Enforce, M03),
        late!(HN_V001, "H-VER-COLLISION", Error, Enforce, M02), late!(HN_V002, "H-VER-GAP", Warn, Warn, M02),
        late!(HN_V003, "H-SEAL-TAMPER", Error, Enforce, M02), late!(HN_V004, "H-SYNC-ANCESTOR", Info, Warn, M02),
        late!(HN_V005, "H-TX-INCOMPLETE", Error, Enforce, M02), late!(HN_V006, "H-ID-COLLISION", Error, Enforce, M02),
        late!(HN_V007, "H-ORD-DUP", Error, Enforce, M02), late!(HN_V008, "H-DOC-BROAD", Warn, Warn, M02),
        late!(HN_V009, "H-DOC-EMPTY", Info, Warn, M02), late!(HN_V010, "H-GIT-SHALLOW", Warn, Warn, M02),
        late!(HN_V011, "H-UPG-PARTIAL", Error, Enforce, M02), late!(HN_V012, "H-REQ-CHANGED", Info, Warn, M03),
        late!(HN_V014, "H-UPG-BOUNDARY", Info, Warn, M03), late!(HN_V015, "H-HOTFIX-DEBT", Warn, Warn, M02),
        late!(HN_V016, "H-ESC-STALE", Warn, Warn, M02),
        late!(HN_E001, "H-ENV-GATE", Error, Enforce, M02), late!(HN_E002, "H-ENV-UNDECLARED", Warn, Warn, M02),
        late!(HN_E003, "H-ENV-UNBOUND", Info, Warn, M02), late!(HN_E004, "H-VOC-UNKNOWN", Error, Enforce, M02),
        late!(HN_E005, "H-MAT-MISSING", Warn, Warn, M02), late!(HN_E006, "H-MAT-ORDER", Error, Enforce, M02),
        late!(HN_E007, "H-MAT-REGRESS", Warn, Warn, M02),
        late!(HN_L001, "H-LC-STALE-PLAN", Warn, Warn, M02), late!(HN_L002, "H-GRAVE-MATCH", Warn, Warn, M02),
        late!(HN_H001, "hier.single-container", Error, Enforce, M03), late!(HN_H002, "hier.parent-cycle", Error, Enforce, M03),
        late!(HN_H003, "hier.orphan-file", Error, Enforce, M03), late!(HN_H004, "hier.orphan-unit", Error, Enforce, M03),
        late!(HN_H005, "hier.hollow", Warn, Warn, M03), late!(HN_H006, "hier.sym-ambiguous", Warn, Warn, M03),
        late!(HN_H007, "hier.rank", Error, Enforce, M03), late!(HN_H008, "hier.dag", Error, Enforce, M03),
        late!(HN_H009, "hier.cycle", Error, Enforce, M03), late!(HN_H010, "hier.visibility", Error, Enforce, M03),
        late!(HN_H011, "hier.uses-noevidence", Warn, Warn, M03), late!(HN_H012, "hier.uses-undeclared", Warn, Warn, M03),
        late!(HN_H013, "hier.promote", Info, Observe, M03), late!(HN_H014, "hier.demote", Info, Observe, M03),
        late!(HN_H015, "hier.split", Info, Observe, M03), late!(HN_H016, "hier.merge", Info, Observe, M03),
        late!(HN_H017, "hier.maturity", Warn, Warn, M03), late!(HN_H018, "hier.stale", Warn, Warn, M03),
        late!(HN_H019, "hier.dormant", Info, Warn, M03), late!(HN_H020, "hier.forgotten", Warn, Warn, M03),
        late!(HN_H021, "hier.bridge-end-missing", Error, Enforce, M03), late!(HN_H022, "hier.bridge-pin", Error, Enforce, M03),
        late!(HN_H023, "hier.double-binding", Error, Enforce, M03), late!(HN_H024, "hier.unverifiable", Warn, Warn, M03),
        late!(HN_H025, "hier.bridge-wire", Error, Enforce, M03),
        late!(HN_W001, "var.request-stale", Warn, Warn, M03), late!(HN_W002, "var.adapter-bypass", Error, Enforce, M03),
        late!(HN_W003, "var.fork-suspect", Warn, Observe, M03), late!(HN_W004, "var.axis-suggest", Info, Observe, M03),
        late!(HN_M004, "DUP-CLAIM", Error, Enforce, M03), late!(HN_M005, "UNSATISFIED", Error, Enforce, M03),
        late!(HN_M006, "NOT-CHECKED", Warn, Warn, M03), late!(HN_M007, "DANGLING", Error, Enforce, M02),
        late!(HN_M008, "MOVE-INCOMPLETE", Error, Enforce, M02), late!(HN_M009, "PKG-HINT", Info, Warn, M03),
        late!(HN_M010, "PROJECTION-OUTSIDE", Error, Enforce, M03), late!(HN_M011, "DETACHED", Info, Warn, M02),
        late!(HN_M012, "LOCK-STALE", Warn, Warn, M03),
    ];
    for s in &mut v { s.floor = is_floor(s.code); s.waivable = WAIVABLE.contains(&s.code); }
    v.sort_by(|a, b| a.code.cmp(&b.code));
    v
});

pub fn all_specs() -> &'static [DiagnosticSpec] { TABLE.as_slice() }
pub fn spec(code: DiagCode) -> Option<&'static DiagnosticSpec> {
    TABLE.binary_search_by(|s| s.code.cmp(&code)).ok().and_then(|i| TABLE.get(i))
}
pub fn is_active(code: DiagCode) -> bool {
    spec(code).is_some_and(|s| rank(s.milestone) <= rank(ACTIVE_MILESTONE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes as c;
    use Severity::{Error as E, Info as I, Warn as W};
    use Stage::{Enforce as En, Observe as Ob, Warn as Wn};

    const R01: &[(DiagCode, Severity, Stage)] = &[
        (c::HN_S001, E, En), (c::HN_S002, E, En), (c::HN_S004, E, En), (c::HN_S005, W, Wn),
        (c::HN_S006, E, En), (c::HN_S007, E, En), (c::HN_S008, E, En), (c::HN_S009, E, En),
        (c::HN_S010, W, Wn), (c::HN_S011, W, Wn), (c::HN_S012, E, En), (c::HN_S013, E, En),
        (c::HN_S014, W, Wn), (c::HN_S015, W, Wn), (c::HN_S016, E, En),
        (c::HN_I001, I, Wn), (c::HN_I002, I, Wn), (c::HN_I003, W, Wn),
        (c::HN_N001, I, Wn), (c::HN_N002, I, Ob),
        (c::HN_R001, E, En), (c::HN_R002, W, Ob), (c::HN_R005, E, En), (c::HN_R007, E, En),
        (c::HN_D001, E, En), (c::HN_D002, E, En),
        (c::HN_T001, E, Ob), (c::HN_T002, E, En), (c::HN_T003, E, En), (c::HN_T004, E, En), (c::HN_T005, E, En),
        (c::HN_U001, W, Ob),
        (c::HN_G001, E, Wn), (c::HN_G002, E, En), (c::HN_G004, W, Wn), (c::HN_G005, E, En),
        (c::HN_X001, E, En), (c::HN_X002, E, Wn), (c::HN_X003, E, En), (c::HN_X004, W, Wn),
        (c::HN_X005, W, Wn), (c::HN_X006, E, En), (c::HN_X007, W, Wn), (c::HN_X008, I, Wn),
        (c::HN_X009, I, Wn), (c::HN_X010, W, Wn), (c::HN_X011, W, Wn), (c::HN_X012, E, En),
        (c::HN_X014, E, En),
        (c::HN_B001, I, Wn), (c::HN_B002, E, En),
        (c::HN_A001, E, Wn), (c::HN_A002, E, Wn), (c::HN_A004, W, Wn),
        (c::HN_C001, E, En), (c::HN_C002, W, Wn), (c::HN_C003, E, En), (c::HN_C004, E, En),
        (c::HN_C005, E, En), (c::HN_C006, E, En), (c::HN_C007, E, En), (c::HN_C008, E, Wn),
        (c::HN_C009, E, En), (c::HN_C011, E, En), (c::HN_C012, E, En), (c::HN_C013, E, En),
        (c::HN_F001, E, En), (c::HN_P001, E, En), (c::HN_V013, W, Wn),
        (c::HN_M001, W, Wn), (c::HN_M002, E, En), (c::HN_M003, E, En),
    ];

    #[test]
    fn r_arch_055__every_0_1_row_has_a_registry_entry_with_its_default() {
        assert_eq!(R01.len(), 72);
        for (code, sev, stage) in R01 {
            let s = spec(*code).unwrap_or_else(|| panic!("{} missing", code.0));
            assert_eq!((s.default_severity, s.default_stage), (*sev, *stage), "{}", code.0);
            assert!(matches!(s.milestone, Milestone::M01), "{}", code.0);
            assert!(is_active(*code), "{}", code.0);
        }
    }

    #[test]
    fn r_chk_042__table_is_sorted_unique_and_the_floor_column_matches() {
        let all = all_specs();
        assert!(all.windows(2).all(|w| w[0].code < w[1].code));
        for s in all { assert_eq!(s.floor, is_floor(s.code), "{}", s.code.0); }
        let waivable: Vec<_> = all.iter().filter(|s| s.waivable).map(|s| s.code).collect();
        assert_eq!(waivable, vec![c::HN_A001, c::HN_A002, c::HN_R002, c::HN_T001, c::HN_U001]);
    }

    #[test]
    fn r_arch_055__later_milestone_codes_are_registered_but_inactive() {
        for code in [c::HN_R004, c::HN_D003, c::HN_V001, c::HN_H001, c::HN_W001, c::HN_E002, c::HN_X013, c::HN_T008, c::HN_M004] {
            assert!(spec(code).is_some(), "{} missing", code.0);
            assert!(!is_active(code), "{} must be inactive in 0.1", code.0);
        }
    }

    #[test]
    fn r_chk_055__unapproved_exception_is_error_at_stage_warn() {
        let s = spec(c::HN_X002).unwrap();
        assert_eq!((s.default_severity, s.default_stage), (Severity::Error, Stage::Warn));
    }
}
