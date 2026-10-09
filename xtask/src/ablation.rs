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

/// Declared values found in added lines of one diff. Literal search only; `fairlead check`
/// is never called (R-ROAD-016). `declared` holds (id, value) pairs.
pub fn hardcoded_count(diff: &str, declared: &[(String, String)]) -> usize {
    diff.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .map(|l| {
            declared
                .iter()
                .filter(|(_, v)| contains_token(l, v))
                .count()
        })
        .sum()
}

/// `value` occurs in `line` not glued to a word character on either side, so `25` is not
/// found inside `#2563eb` or `250ms`.
fn contains_token(line: &str, value: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    line.match_indices(value).any(|(i, _)| {
        !word(line[..i].chars().next_back()) && !word(line[i + value.len()..].chars().next())
    })
}

/// Percent reduction of arm C against arm A, from mean counts per arm.
pub fn reduction_pct(mean_a: f64, mean_c: f64) -> f64 {
    if mean_a == 0.0 {
        0.0
    } else {
        (1.0 - mean_c / mean_a) * 100.0
    }
}

/// The decision rule of the protocol (E2). Arm B is never passed to this function.
pub fn verdict(reduction: f64) -> &'static str {
    if reduction >= PASS_REDUCTION_PCT as f64 {
        "pass"
    } else if reduction < WEAK_BELOW_PCT as f64 {
        "weak: kill assumption 2"
    } else {
        "inconclusive: between 20 and 50 percent"
    }
}

#[derive(serde::Deserialize)]
struct DeclaredFile {
    items: Vec<String>,
}

/// One declared item of `ablation/declared.toml` (`id|value|property`).
pub struct Declared {
    pub id: String,
    pub value: String,
    pub property: String,
}

pub fn parse_declared(text: &str) -> Result<Vec<Declared>, String> {
    let file: DeclaredFile = toml::from_str(text).map_err(|e| e.to_string())?;
    file.items
        .iter()
        .map(|item| {
            let mut parts = item.splitn(3, '|');
            match (parts.next(), parts.next(), parts.next()) {
                (Some(id), Some(value), Some(property)) if !value.is_empty() => Ok(Declared {
                    id: id.to_string(),
                    value: value.to_string(),
                    property: property.to_string(),
                }),
                _ => Err(format!("declared item {item:?} is not id|value|property")),
            }
        })
        .collect()
}

/// `diff_id,hardcoded` rows for every `*.diff` in `diffs`, sorted by id. The id is the opaque
/// file stem; no arm or task appears (blinding, R-ROAD-016).
pub fn score_dir(diffs: &std::path::Path, declared: &[Declared]) -> Result<String, String> {
    let pairs: Vec<(String, String)> = declared
        .iter()
        .map(|d| (d.id.clone(), d.value.clone()))
        .collect();
    let mut rows = Vec::new();
    for entry in std::fs::read_dir(diffs).map_err(|e| format!("{}: {e}", diffs.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|x| x == "diff") {
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            rows.push((id, hardcoded_count(&text, &pairs)));
        }
    }
    rows.sort();
    let mut out = String::from("diff_id,hardcoded\n");
    for (id, n) in rows {
        out.push_str(&format!("{id},{n}\n"));
    }
    Ok(out)
}

/// Joins scores with the arm map (`diff_id,arm,task`, written after scoring) and reports the
/// mean per arm and track; the verdict compares arm C with arm A only (E2).
pub fn report(scores: &str, map: &str) -> Result<String, String> {
    use std::collections::BTreeMap;
    let rows = |text: &str| -> Vec<Vec<String>> {
        text.lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split(',').map(|c| c.trim().to_string()).collect())
            .collect()
    };
    let mut score = BTreeMap::new();
    for r in rows(scores) {
        let n: f64 = r
            .get(1)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| format!("bad score row {r:?}"))?;
        score.insert(r[0].clone(), n);
    }
    // (arm, track) -> (sum, count)
    let mut acc: BTreeMap<(String, String), (f64, usize)> = BTreeMap::new();
    for r in rows(map) {
        let [id, arm, task] = r.as_slice() else {
            return Err(format!("bad map row {r:?}"));
        };
        let n = *score
            .get(id)
            .ok_or_else(|| format!("{id} is in the map but was not scored"))?;
        let track = if task.starts_with("ui-") {
            "ui"
        } else {
            "backend"
        };
        for key in [
            (arm.clone(), track.to_string()),
            (arm.clone(), "all".to_string()),
        ] {
            let e = acc.entry(key).or_insert((0.0, 0));
            e.0 += n;
            e.1 += 1;
        }
    }
    let mean = |arm: &str, track: &str| {
        acc.get(&(arm.to_string(), track.to_string()))
            .map(|(s, c)| s / *c as f64)
    };
    let mut out = String::from("arm,track,mean_hardcoded\n");
    for ((arm, track), (s, c)) in &acc {
        out.push_str(&format!("{arm},{track},{:.2}\n", s / *c as f64));
    }
    let (Some(a), Some(c)) = (mean("A", "all"), mean("C", "all")) else {
        return Err("the map needs arms A and C".into());
    };
    let r = reduction_pct(a, c);
    out.push_str(&format!(
        "reduction C vs A: {r:.1}%\nverdict: {}\n",
        verdict(r)
    ));
    Ok(out)
}

/// The static AGENTS.md of arm B: the declared facts as prose, one line per item.
pub fn arm_b_agents(declared: &[Declared]) -> String {
    let mut out = String::from(
        "# AGENTS.md\n\nUse these project values by name; do not hardcode their literals.\n\n",
    );
    for d in declared {
        out.push_str(&format!(
            "- `{}` is `{}` (use it for {}).\n",
            d.id, d.value, d.property
        ));
    }
    out
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_road_005__verdict_boundaries() {
        assert_eq!(verdict(50.0), "pass");
        assert_eq!(verdict(49.9), "inconclusive: between 20 and 50 percent");
        assert_eq!(verdict(19.9), "weak: kill assumption 2");
        assert_eq!(reduction_pct(10.0, 5.0), 50.0);
    }

    #[test]
    fn r_road_016__scorer_counts_literal_values_in_added_lines_only() {
        let declared = vec![("tok.color.primary".to_string(), "#2563eb".to_string())];
        let diff = "+++ b/a.tsx\n+color: \"#2563eb\"\n-color: \"#2563eb\"\n context #2563eb\n";
        assert_eq!(hardcoded_count(diff, &declared), 1);
    }

    #[test]
    fn r_road_016__short_values_match_whole_tokens_only() {
        let declared = vec![
            ("const.note.page-size".to_string(), "25".to_string()),
            ("const.sync.retry-max".to_string(), "5".to_string()),
        ];
        assert_eq!(
            hardcoded_count("+color: #2563eb; delay: 250ms\n", &declared),
            0
        );
        assert_eq!(hardcoded_count("+const PAGE: usize = 25;\n", &declared), 1);
    }

    #[test]
    fn r_road_016__score_output_is_blind_and_report_joins_after() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("k3.diff"), "+a #2563eb\n+b #2563eb\n").unwrap();
        std::fs::write(dir.path().join("q9.diff"), "+a var(--primary)\n").unwrap();
        let declared = parse_declared("items = [\"tok.color.primary|#2563eb|color\"]").unwrap();
        let scores = score_dir(dir.path(), &declared).unwrap();
        assert_eq!(scores, "diff_id,hardcoded\nk3,2\nq9,0\n");
        let report = report(&scores, "diff_id,arm,task\nk3,A,ui-01\nq9,C,ui-01\n").unwrap();
        assert!(report.contains("reduction C vs A: 100.0%"), "{report}");
        assert!(report.ends_with("verdict: pass\n"), "{report}");
    }

    #[test]
    fn r_road_005__repository_task_list_and_declared_items_match_the_protocol() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ablation");
        let declared =
            parse_declared(&std::fs::read_to_string(root.join("declared.toml")).unwrap()).unwrap();
        assert_eq!(declared.len(), 30);
        let tasks = std::fs::read_to_string(root.join("tasks.toml")).unwrap();
        assert_eq!(tasks.matches("track = \"ui\"").count(), UI_TASKS);
        assert_eq!(tasks.matches("track = \"backend\"").count(), BACKEND_TASKS);
        assert!(arm_b_agents(&declared).contains("`tok.color.primary` is `#2563eb`"));
    }

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
