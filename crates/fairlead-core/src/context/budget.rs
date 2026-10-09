//! Priority packing under `--budget` (R-CTX-040, R-CTX-041, R-CTX-046).
use std::collections::BTreeMap;

use crate::context::brief::{Block, ContextStatus, Entry, Omission};
use crate::context::priority::{Candidate, Priority, rank_cmp};

pub struct Packed {
    pub kept: Vec<Entry>,
    pub omitted: Vec<Omission>,
    pub status: ContextStatus,
}

/// One `+N more` per block: pre-omissions (top-K caps) and packing drops add up; the explain target is the
/// first pre-omission's, else the best-ranked dropped object of that block.
pub fn merge_omissions(pre: &[Omission], dropped: &[Candidate]) -> Vec<Omission> {
    let mut by: BTreeMap<Block, (u32, Option<String>)> = BTreeMap::new();
    for o in pre {
        let e = by.entry(o.block).or_insert((0, None));
        e.0 += o.count;
        if e.1.is_none() {
            e.1 = Some(o.explain.clone());
        }
    }
    let mut ranked: Vec<&Candidate> = dropped.iter().collect();
    ranked.sort_by(|a, b| rank_cmp(a, b));
    for c in ranked {
        let e = by.entry(c.block).or_insert((0, None));
        e.0 += 1;
        if e.1.is_none() {
            e.1 = Some(c.explain.clone());
        }
    }
    by.into_iter()
        .map(|(block, (count, ex))| Omission {
            block,
            count,
            explain: ex.unwrap_or_default(),
        })
        .collect()
}

pub fn pack(
    mut cands: Vec<Candidate>,
    pre: Vec<Omission>,
    cap: Option<u32>,
    base: u32,
    cost: &dyn Fn(&Entry) -> u32,
    measure: &dyn Fn(&[Entry], &[Omission]) -> u32,
) -> Packed {
    cands.sort_by(rank_cmp);
    let Some(cap) = cap else {
        return Packed {
            kept: cands.into_iter().map(|c| c.entry).collect(),
            omitted: merge_omissions(&pre, &[]),
            status: ContextStatus::Ok,
        };
    };
    let (mut kept, optional): (Vec<Candidate>, Vec<Candidate>) =
        cands.into_iter().partition(|c| c.priority.is_mandatory());
    let mandatory_len = kept.len();
    let mut used = kept
        .iter()
        .fold(base, |acc, c| acc.saturating_add(cost(&c.entry)));
    let mut dropped: Vec<Candidate> = Vec::new();
    let mut closed_at: Option<Priority> = None;
    for c in optional {
        if closed_at.is_some_and(|p| c.priority > p) {
            dropped.push(c);
            continue;
        }
        let add = cost(&c.entry);
        if used.saturating_add(add) <= cap {
            used = used.saturating_add(add);
            kept.push(c);
        } else {
            closed_at.get_or_insert(c.priority);
            dropped.push(c);
        }
    }
    loop {
        let om = merge_omissions(&pre, &dropped);
        let entries: Vec<Entry> = kept.iter().map(|c| c.entry.clone()).collect();
        if measure(&entries, &om) <= cap {
            return Packed {
                kept: entries,
                omitted: om,
                status: ContextStatus::Ok,
            };
        }
        if kept.len() > mandatory_len {
            if let Some(c) = kept.pop() {
                dropped.push(c);
            }
            continue;
        }
        let min_budget = measure(&entries, &om);
        return Packed {
            kept: entries.iter().map(Entry::to_l0).collect(),
            omitted: om,
            status: ContextStatus::TruncatedMandatory { min_budget },
        };
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::context::brief::{Block, BriefRule};
    use crate::context::n32;
    use crate::context::priority::{Priority, Relevance, Score};

    fn cost(e: &Entry) -> u32 {
        n32(e.cost_bytes()).div_ceil(4)
    }

    fn measure(k: &[Entry], o: &[Omission]) -> u32 {
        k.iter().map(cost).sum::<u32>()
            + o.iter()
                .map(|x| cost(&Entry::Omitted(x.clone())))
                .sum::<u32>()
    }

    /// A note whose cost in tokens is `ceil((7 + len) / 4)`.
    fn note(prio: Priority, sev_weight: u32, key: &str, len: usize) -> Candidate {
        Candidate {
            priority: prio,
            score: Score::new(sev_weight, Relevance::Direct, 0),
            hint: 0,
            key: key.into(),
            explain: key.into(),
            block: Block::Items,
            entry: Entry::Note {
                text: "x".repeat(len),
            },
        }
    }

    fn run(c: Vec<Candidate>, cap: Option<u32>) -> Packed {
        pack(c, vec![], cap, 0, &cost, &measure)
    }

    #[test]
    fn r_ctx_040__lower_priorities_are_dropped_first_and_the_drop_is_counted() {
        let p = run(
            vec![
                note(Priority::P1ErrorRules, 4, "k1", 13), // 5 tokens, mandatory
                note(Priority::P3Items, 1, "k3", 33),      // 10
                note(Priority::P4WarnRules, 2, "k4", 33),  // 10
                note(Priority::P6SiblingsChildren, 1, "k6", 33), // 10
            ],
            Some(30),
        );
        assert_eq!(p.status, ContextStatus::Ok);
        assert_eq!(p.kept.len(), 2);
        assert_eq!(
            p.omitted,
            vec![Omission {
                block: Block::Items,
                count: 2,
                explain: "k4".into()
            }]
        );
        assert!(measure(&p.kept, &p.omitted) <= 30);
    }

    #[test]
    fn r_ctx_046__the_overflow_answer_is_a_plus_n_more_line_not_a_continuation() {
        let p = run(
            vec![
                note(Priority::P1ErrorRules, 4, "k1", 13),
                note(Priority::P3Items, 1, "k3", 80),
            ],
            Some(20),
        );
        assert_eq!(
            p.omitted.first().map(Omission::line).as_deref(),
            Some("+1 more -> fairlead explain k3")
        );
    }

    #[test]
    fn r_ctx_040__a_drop_in_a_higher_class_closes_every_lower_class() {
        let p = run(
            vec![
                note(Priority::P1ErrorRules, 4, "k1", 13),
                note(Priority::P3Items, 1, "big", 80), // 22 tokens: does not fit
                note(Priority::P6SiblingsChildren, 1, "tiny", 1), // 2 tokens: would fit, but is lower priority
            ],
            Some(20),
        );
        assert_eq!(p.kept.len(), 1);
        assert_eq!(p.omitted.first().map(|o| o.count), Some(2));
    }

    #[test]
    fn r_ctx_040__a_smaller_item_of_the_same_class_may_still_fit() {
        let p = run(
            vec![
                note(Priority::P1ErrorRules, 4, "k1", 13),
                note(Priority::P3Items, 4, "big", 80),
                note(Priority::P3Items, 1, "small", 1),
            ],
            Some(20),
        );
        let texts: Vec<String> = p.kept.iter().map(Entry::standalone).collect();
        assert_eq!(texts.len(), 2);
        assert_eq!(
            p.omitted.first().map(|o| (o.count, o.explain.as_str())),
            Some((1, "big"))
        );
    }

    #[test]
    fn r_ctx_040__unlimited_keeps_everything_and_reports_only_pre_omissions() {
        let pre = vec![Omission {
            block: Block::Items,
            count: 3,
            explain: "tok.z".into(),
        }];
        let p = pack(
            vec![
                note(Priority::P1ErrorRules, 4, "k1", 13),
                note(Priority::P7AncestorL1, 1, "k7", 400),
            ],
            pre.clone(),
            None,
            0,
            &cost,
            &measure,
        );
        assert_eq!(p.kept.len(), 2);
        assert_eq!(p.omitted, pre);
        assert_eq!(p.status, ContextStatus::Ok);
    }

    #[test]
    fn r_ctx_040__pre_omissions_merge_with_packing_drops_per_block() {
        let pre = [Omission {
            block: Block::Items,
            count: 3,
            explain: "tok.z".into(),
        }];
        let dropped = [note(Priority::P3Items, 1, "tok.a", 5)];
        let m = merge_omissions(&pre, &dropped);
        assert_eq!(
            m,
            vec![Omission {
                block: Block::Items,
                count: 4,
                explain: "tok.z".into()
            }]
        );
    }

    #[test]
    fn r_ctx_041__mandatory_overflow_is_emitted_in_l0_form_with_the_minimum_budget() {
        let rule = |id: &str| BriefRule {
            id: id.into(),
            sev: "error".into(),
            stage: "enforce".into(),
            home: "root".into(),
            inherited: false,
            glob: vec!["**/*.tsx".into()],
            why: Some(format!("data fairlead/control.toml@000000 \"{id}\"")),
            ovr: None,
            l0: false,
        };
        let mk = |id: &str| Candidate {
            priority: Priority::P1ErrorRules,
            score: Score::new(4, Relevance::Glob, 0),
            hint: 0,
            key: id.into(),
            explain: id.into(),
            block: Block::Rules,
            entry: Entry::Rule(rule(id)),
        };
        let p = run(
            vec![
                mk("rule.a"),
                mk("rule.b"),
                note(Priority::P3Items, 1, "tok.x", 10),
            ],
            Some(3),
        );
        let normal = [Entry::Rule(rule("rule.a")), Entry::Rule(rule("rule.b"))];
        let om = vec![Omission {
            block: Block::Items,
            count: 1,
            explain: "tok.x".into(),
        }];
        assert_eq!(
            p.status,
            ContextStatus::TruncatedMandatory {
                min_budget: measure(&normal, &om)
            }
        );
        assert!(measure(&normal, &om) > 3);
        assert_eq!(p.omitted, om);
        assert!(p.kept.iter().all(|e| matches!(e, Entry::Rule(r) if r.l0)));
        assert_eq!(p.kept.len(), 2);
    }

    #[test]
    fn r_ctx_043__a_packed_brief_never_exceeds_its_cap() {
        for cap in 8..=120u32 {
            let c = vec![
                note(Priority::P1ErrorRules, 4, "k1", 13),
                note(Priority::P2Drift, 4, "k2", 20),
                note(Priority::P3Items, 1, "k3", 33),
                note(Priority::P3Items, 1, "k3b", 12),
                note(Priority::P4WarnRules, 2, "k4", 33),
                note(Priority::P6SiblingsChildren, 1, "k6", 33),
                note(Priority::P7AncestorL1, 1, "k7", 33),
            ];
            let p = run(c, Some(cap));
            match p.status {
                ContextStatus::Ok => assert!(measure(&p.kept, &p.omitted) <= cap, "cap {cap}"),
                ContextStatus::TruncatedMandatory { min_budget } => {
                    assert!(min_budget > cap, "cap {cap}")
                }
            }
        }
    }
}
