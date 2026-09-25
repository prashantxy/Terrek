use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// Completions for the palette: known command names first, then things typed before.
pub struct SuggestionEngine {
    commands: Vec<String>,
    history: Vec<String>,
}

impl SuggestionEngine {
    pub fn new(commands: Vec<String>) -> Self {
        Self {
            commands,
            history: Vec::new(),
        }
    }

    pub fn remember(&mut self, entry: &str) {
        let entry = entry.trim();
        if entry.is_empty() {
            return;
        }
        self.history.retain(|h| h != entry);
        self.history.push(entry.to_string());
    }

    /// The best entry that extends `input`, for ghost text and Tab.
    pub fn complete(&self, input: &str) -> Option<String> {
        if input.is_empty() {
            return None;
        }
        let extends = |s: &&String| s.starts_with(input) && s.len() > input.len();
        self.history
            .iter()
            .rev()
            .find(extends)
            .or_else(|| self.commands.iter().find(extends))
            .cloned()
    }

    /// Closest command names, for "did you mean".
    pub fn fuzzy(&self, input: &str, limit: usize) -> Vec<String> {
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(i64, &String)> = self
            .commands
            .iter()
            .filter_map(|c| matcher.fuzzy_match(c, input).map(|s| (s, c)))
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, c)| c.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_recent_history_over_commands() {
        let mut engine = SuggestionEngine::new(vec!["ask".into(), "history".into()]);
        assert_eq!(engine.complete("as").as_deref(), Some("ask"));
        engine.remember("ask why is my build slow");
        assert_eq!(
            engine.complete("as").as_deref(),
            Some("ask why is my build slow")
        );
        assert_eq!(
            engine.complete("ask"),
            Some("ask why is my build slow".into())
        );
        assert_eq!(engine.complete(""), None);
        assert_eq!(
            engine.complete("history"),
            None,
            "no suggestion when already complete"
        );
    }

    #[test]
    fn fuzzy_finds_typos() {
        let engine = SuggestionEngine::new(vec!["history".into(), "search".into()]);
        assert_eq!(engine.fuzzy("hstry", 1), vec!["history".to_string()]);
    }
}
