use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

use crate::entry::Entry;
use crate::usage::Usage;

const NAME_BOOST: u32 = 8;

pub fn rank(query: &str, apps: &[Entry], limit: usize, usage: &Usage, freq_on: bool) -> Vec<usize> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut scored: Vec<(u32, usize)> = Vec::new();
    for (i, app) in apps.iter().enumerate() {
        if let Some(score) = best_score(&pattern, &mut matcher, app) {
            let boost = if freq_on { usage.boost(&app.id) } else { 0 };
            scored.push((score + boost, i));
        }
    }

    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| apps[a.1].name.cmp(&apps[b.1].name))
    });
    scored.truncate(limit);
    scored.into_iter().map(|(_, i)| i).collect()
}

fn best_score(pattern: &Pattern, matcher: &mut Matcher, app: &Entry) -> Option<u32> {
    let mut best = pattern
        .score(app.search_name.slice(..), matcher)
        .map(|s| s + NAME_BOOST);

    if let Some(generic) = &app.search_generic_name {
        best = max_opt(best, pattern.score(generic.slice(..), matcher));
    }
    for keyword in &app.search_keywords {
        best = max_opt(best, pattern.score(keyword.slice(..), matcher));
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
