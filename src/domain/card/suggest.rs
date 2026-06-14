//! Did-you-mean suggestions for failed id lookups. The near-match is only
//! ever surfaced as a hint -- ids are never prefix- or fuzzy-resolved.

/// Edit-distance budget for a plausible typo: a mistyped `-hex4` nonce or a
/// couple of slipped characters qualify; a different slug does not.
fn typo_budget(len: usize) -> usize {
    (len / 4).max(2)
}

/// The closest candidate id to a failed lookup, or `None` when nothing is
/// near enough to plausibly be the id the caller meant. Ties break
/// lexicographically so the hint is deterministic.
pub fn did_you_mean<'a, I>(target: &str, candidates: I) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let target_lower = target.to_ascii_lowercase();
    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates {
        let budget = typo_budget(target.chars().count().max(candidate.chars().count()));
        let distance = levenshtein(&target_lower, &candidate.to_ascii_lowercase());
        if distance > budget {
            continue;
        }
        let better = match best {
            None => true,
            Some((best_distance, best_id)) => {
                distance < best_distance || (distance == best_distance && candidate < best_id)
            }
        };
        if better {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, id)| id.to_string())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, char_a) in a.iter().enumerate() {
        let mut previous_diagonal = row[0];
        row[0] = i + 1;
        for (j, char_b) in b.iter().enumerate() {
            let cost = usize::from(char_a != char_b);
            let value = (previous_diagonal + cost)
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            previous_diagonal = row[j + 1];
            row[j + 1] = value;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_hash_nonce_finds_the_real_card() {
        let candidates = ["dec-pick-the-parser-9f3c", "dec-adopt-the-funnel-11aa"];
        assert_eq!(
            did_you_mean("dec-pick-the-parser-0000", candidates),
            Some("dec-pick-the-parser-9f3c".to_string())
        );
    }

    #[test]
    fn a_different_slug_is_not_a_typo() {
        let candidates = ["dec-adopt-the-funnel-11aa"];
        assert_eq!(did_you_mean("dec-pick-the-parser-0000", candidates), None);
        assert_eq!(did_you_mean("task-anything", []), None);
    }

    #[test]
    fn ties_break_lexicographically_for_a_deterministic_hint() {
        let candidates = ["task-fix-sync-bb22", "task-fix-sync-aa11"];
        assert_eq!(
            did_you_mean("task-fix-sync-0000", candidates),
            Some("task-fix-sync-aa11".to_string())
        );
    }
}
