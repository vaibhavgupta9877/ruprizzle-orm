//! "Did you mean…?" suggestions.
//!
//! Every diagnostic that names an unknown identifier should offer the closest
//! known one. This is cheap to compute and is a large part of why Prisma's schema
//! errors feel helpful rather than merely correct.

/// Levenshtein edit distance between two strings.
///
/// Case-insensitive, because a schema author writing `datetime` for `DateTime`
/// has made a capitalisation mistake, not a spelling one, and should still get
/// the suggestion.
#[must_use]
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().flat_map(char::to_lowercase).collect();
    let b: Vec<char> = b.chars().flat_map(char::to_lowercase).collect();

    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    // Single-row DP: `prev[j]` is the distance for the previous row of `a`.
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1) // deletion
                .min(curr[j] + 1) // insertion
                .min(prev[j] + cost); // substitution
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}

/// The closest candidate to `input`, if any is close enough to be worth showing.
///
/// The threshold scales with the length of the input: a three-letter typo should
/// not suggest a twelve-letter identifier. Returns `None` rather than a bad guess,
/// because a confidently wrong suggestion is worse than none.
#[must_use]
pub fn closest<'a, I, S>(input: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a S>,
    S: AsRef<str> + 'a + ?Sized,
{
    let max = match input.chars().count() {
        0..=2 => 1,
        3..=5 => 2,
        _ => 3,
    };

    candidates
        .into_iter()
        .map(|c| {
            let c = c.as_ref();
            (distance(input, c), c)
        })
        .filter(|(d, _)| *d <= max)
        .min_by_key(|(d, c)| (*d, c.len()))
        .map(|(_, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_basics() {
        assert_eq!(distance("", ""), 0);
        assert_eq!(distance("abc", "abc"), 0);
        assert_eq!(distance("abc", ""), 3);
        assert_eq!(distance("kitten", "sitting"), 3);
    }

    #[test]
    fn distance_is_case_insensitive() {
        assert_eq!(distance("datetime", "DateTime"), 0);
    }

    #[test]
    fn suggests_the_obvious_typo() {
        let types = ["String", "Int", "BigInt", "Boolean", "DateTime"];
        assert_eq!(closest("Strng", types.iter()), Some("String"));
        assert_eq!(closest("boolean", types.iter()), Some("Boolean"));
    }

    #[test]
    fn declines_to_guess_when_nothing_is_close() {
        let types = ["String", "Int", "Boolean"];
        assert_eq!(closest("Geometry", types.iter()), None);
    }

    #[test]
    fn short_inputs_get_a_tight_threshold() {
        // `Int` is 2 edits from `Uid`, which exceeds the threshold for a
        // three-character input.
        assert_eq!(closest("Uid", ["Int"].iter()), None);
        assert_eq!(closest("Uuid", ["Uuid", "Int"].iter()), Some("Uuid"));
    }
}
