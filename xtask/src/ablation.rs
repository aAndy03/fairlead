//! Pre-registered 0.1 thresholds (R-ROAD-014), bound to docs/plans/0.1/ablation-protocol.md by a test.
#![allow(
    dead_code,
    reason = "read by the ablation, replay and bench commands of P9"
)]

pub const UI_TASKS: usize = 10;
pub const BACKEND_TASKS: usize = 10;
pub const RUNS_PER_TASK: usize = 3;
pub const PASS_REDUCTION_PCT: u32 = 50; // E2 gate, arm C against arm A
pub const WEAK_BELOW_PCT: u32 = 20; // kill assumption 2
pub const HYGIENE_MIN_PCT: u32 = 80; // E3 and R-ROAD-009
pub const HYGIENE_TASKS: usize = 12; // roadmap 7.2
pub const PRECISION_SAMPLE: usize = 200; // R-CHK-051, R-ROAD-003
pub const PRECISION_MAX_FALSE_PCT: u32 = 2;
pub const REPLAY_INTRODUCING: usize = 50; // R-ROAD-004
pub const REPLAY_RECALL_MIN_PCT: u32 = 70;
pub const REPLAY_UNRELATED: usize = 20;
pub const REPLAY_FP_MAX_PCT: u32 = 5;
pub const SYNTAX_CARD_SAMPLE: usize = 100; // E8
pub const SYNTAX_CARD_MAX_INVALID_PCT: u32 = 5;
pub const ADOPTION_MAX_MINUTES: u32 = 15; // R-ROAD-006
pub const E6_COLD_CHECK_MAX_S: u64 = 60;
pub const E6_WARM_CHECK_MAX_S: u64 = 5;
pub const E6_CHANGED_MAX_S: u64 = 1;
pub const E6_PEAK_RSS_MAX_MB: u64 = 500;

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    fn protocol() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/plans/0.1/ablation-protocol.md"
        );
        std::fs::read_to_string(path).unwrap_or_default()
    }

    fn value_of(doc: &str, key: &str) -> Option<u64> {
        let prefix = format!("threshold.{key} = ");
        doc.lines()
            .find_map(|l| l.strip_prefix(&prefix))
            .and_then(|v| v.trim().parse().ok())
    }

    #[test]
    fn r_road_014__thresholds_match_committed_protocol() {
        let doc = protocol();
        let pairs: [(&str, u64); 19] = [
            ("pass_reduction_pct", PASS_REDUCTION_PCT as u64),
            ("weak_below_pct", WEAK_BELOW_PCT as u64),
            ("hygiene_min_pct", HYGIENE_MIN_PCT as u64),
            ("precision_max_false_pct", PRECISION_MAX_FALSE_PCT as u64),
            ("precision_sample", PRECISION_SAMPLE as u64),
            ("replay_recall_min_pct", REPLAY_RECALL_MIN_PCT as u64),
            ("replay_fp_max_pct", REPLAY_FP_MAX_PCT as u64),
            ("replay_introducing", REPLAY_INTRODUCING as u64),
            ("replay_unrelated", REPLAY_UNRELATED as u64),
            (
                "syntax_card_max_invalid_pct",
                SYNTAX_CARD_MAX_INVALID_PCT as u64,
            ),
            ("syntax_card_sample", SYNTAX_CARD_SAMPLE as u64),
            ("adoption_max_minutes", ADOPTION_MAX_MINUTES as u64),
            ("e6_cold_check_max_s", E6_COLD_CHECK_MAX_S),
            ("e6_warm_check_max_s", E6_WARM_CHECK_MAX_S),
            ("e6_changed_max_s", E6_CHANGED_MAX_S),
            ("e6_peak_rss_max_mb", E6_PEAK_RSS_MAX_MB),
            ("ui_tasks", UI_TASKS as u64),
            ("backend_tasks", BACKEND_TASKS as u64),
            ("runs_per_task", RUNS_PER_TASK as u64),
        ];
        for (key, want) in pairs {
            assert_eq!(
                value_of(&doc, key),
                Some(want),
                "threshold.{key} not committed"
            );
        }
    }
}
