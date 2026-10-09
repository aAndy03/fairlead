//! Floor rules: compiled in, not configurable by any repository file (R-AGT-009, R-CHK-008).
use fairlead_syntax::diag::{DiagCode, Severity};
use crate::{config::Stage, diag::codes as c};

const FLOOR: [DiagCode; 9] = [c::HN_D001, c::HN_X001, c::HN_X003, c::HN_B002, c::HN_F001,
                              c::HN_P001, c::HN_C001, c::HN_C003, c::HN_C004];

pub fn is_floor(code: DiagCode) -> bool { FLOOR.contains(&code) }

/// A floor code is never below its default severity and always at `enforce`.
pub fn clamp(code: DiagCode, severity: Severity, stage: Stage, default_severity: Severity) -> (Severity, Stage) {
    if is_floor(code) { (severity.max(default_severity), Stage::Enforce) } else { (severity, stage) }
}

/// May an `ovr`, `[[allow]]`, baseline or rule key change this code's severity, stage or waivability?
pub fn tunable(code: DiagCode) -> bool { !is_floor(code) }

#[cfg(test)]
mod tests {
use super::*;
use crate::diag::codes as c;

#[test]
fn r_chk_008__floor_codes_are_exactly_the_f_column_of_chapter_13() {
    for code in [c::HN_D001, c::HN_X001, c::HN_X003, c::HN_B002, c::HN_F001, c::HN_P001,
                 c::HN_C001, c::HN_C003, c::HN_C004] {
        assert!(is_floor(code), "{} must be floor", code.0);
    }
    for code in [c::HN_T001, c::HN_G001, c::HN_X002, c::HN_R001, c::HN_C002, c::HN_A001] {
        assert!(!is_floor(code), "{} must not be floor", code.0);
    }
}

#[test]
fn r_agt_009__clamp_never_lowers_a_floor_rule() {
    assert_eq!(clamp(c::HN_F001, Severity::Info, Stage::Observe, Severity::Error), (Severity::Error, Stage::Enforce));
    assert_eq!(clamp(c::HN_T001, Severity::Info, Stage::Observe, Severity::Error), (Severity::Info, Stage::Observe));
}

#[test]
fn r_scope_039__floor_codes_are_not_tunable() {
    assert!(!tunable(c::HN_X001));
    assert!(tunable(c::HN_T001));
}
}
