//! Fuzzy ranking of apps against the query, using nucleo (Helix's matcher).

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::apps::AppEntry;
use crate::usage::Usage;

/// Small boost so a name hit outranks a keyword-only hit of equal raw score.
const NAME_BOOST: u32 = 8;

/// Rank apps against `query`, returning indices into `apps`, best first, capped at
/// `limit`. An empty/whitespace query yields no results (results appear only when
/// something is typed). When `freq_on`, a usage-based boost nudges frequently- and
/// recently-launched apps upward.
pub fn rank(query: &str, apps: &[AppEntry], limit: usize, usage: &Usage, freq_on: bool) -> Vec<usize> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf: Vec<char> = Vec::new();

    let mut scored: Vec<(u32, usize)> = Vec::new();
    for (i, app) in apps.iter().enumerate() {
        if let Some(score) = best_score(&pattern, &mut matcher, &mut buf, app) {
            let boost = if freq_on { usage.boost(&app.desktop_id) } else { 0 };
            scored.push((score + boost, i));
        }
    }

    // Highest score first; tie-break by name for stable, alphabetical ordering.
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| apps[a.1].name.cmp(&apps[b.1].name))
    });
    scored.truncate(limit);
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Best score across the app's searchable fields (name is boosted).
fn best_score(
    pattern: &Pattern,
    matcher: &mut Matcher,
    buf: &mut Vec<char>,
    app: &AppEntry,
) -> Option<u32> {
    let mut best = pattern
        .score(Utf32Str::new(&app.name, buf), matcher)
        .map(|s| s + NAME_BOOST);

    if let Some(generic) = &app.generic_name {
        best = max_opt(best, pattern.score(Utf32Str::new(generic, buf), matcher));
    }
    for kw in &app.keywords {
        best = max_opt(best, pattern.score(Utf32Str::new(kw, buf), matcher));
    }
    best
}

fn max_opt(a: Option<u32>, b: Option<u32>) -> Option<u32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (Some(x), None) => Some(x),
        (None, b) => b,
    }
}
