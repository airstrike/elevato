//! Rhai syntax highlighting for the editor — rung (a) of the plan's
//! ladder: iced's [`highlighter::Highlighter`] trait implemented over
//! syntect's parser, loading the embedded `assets/rhai.sublime-syntax`
//! grammar. Instead of syntect's theme machinery, each parsed scope is
//! classified into a [`Kind`] and colored from [`crate::theme`]'s
//! palette at format time, so tokens follow the app's light/dark theme
//! without the highlighter having to know which theme is active.
//!
//! The incremental caching (snapshots every [`LINES_PER_SNAPSHOT`]
//! lines, `current_line` tracking) mirrors `iced_highlighter`'s own
//! implementation.

use std::ops::Range;
use std::sync::LazyLock;

use iced::advanced::text::highlighter;
use iced::{Font, Theme};
use syntect::parsing::{
    ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxDefinition, SyntaxReference, SyntaxSet,
    SyntaxSetBuilder,
};

use crate::theme;

/// The embedded Rhai grammar, compiled into the binary so the wasm
/// build needs no asset fetching.
const GRAMMAR: &str = include_str!("../assets/rhai.sublime-syntax");

/// Lines per incremental parse snapshot (same rhythm as
/// `iced_highlighter`): editing line N re-parses from the snapshot at
/// the previous multiple of this, not from the top.
const LINES_PER_SNAPSHOT: usize = 50;

/// The syntax set, built exactly once — syntax-set construction is far
/// too expensive for anything per-frame or per-keystroke.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(|| {
    let mut builder = SyntaxSetBuilder::new();
    builder.add_plain_text_syntax();
    // `false`: the editor hands lines over without trailing newlines.
    // A malformed grammar degrades to plain text rather than panicking;
    // the tests below pin that it actually loads.
    if let Ok(definition) = SyntaxDefinition::load_from_str(GRAMMAR, false, Some("Rhai")) {
        builder.add(definition);
    }
    builder.build()
});

/// Scope-prefix → [`Kind`], checked in order; first match wins. Walked
/// per stack scope from the innermost scope outwards.
static KINDS: LazyLock<[(Scope, Kind); 7]> = LazyLock::new(|| {
    let scope = |name: &str| Scope::new(name).expect("invariant: the scope literal parses");
    [
        (scope("comment"), Kind::Comment),
        (scope("string"), Kind::String),
        (scope("constant.numeric"), Kind::Number),
        (scope("constant.language"), Kind::Constant),
        (scope("constant.character"), Kind::String),
        (scope("keyword.operator"), Kind::Operator),
        (scope("keyword"), Kind::Keyword),
    ]
});

/// What a highlighted token is. The editor's format callback colors
/// each kind from the current theme's palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Line and block comments.
    Comment,
    /// String literals, including escapes.
    String,
    /// Numeric literals.
    Number,
    /// `true` and `false`.
    Constant,
    /// Operators and punctuation, including closure pipes.
    Operator,
    /// Declaration and control keywords.
    Keyword,
}

impl Kind {
    /// The text format for this kind under `theme` — a foreground
    /// color only; the font stays the editor's monospace.
    pub fn format(&self, theme: &Theme) -> highlighter::Format<Font> {
        let palette = theme::palette(theme);
        let color = match self {
            Kind::Comment => palette.syntax_comment,
            Kind::String => palette.syntax_string,
            Kind::Number => palette.syntax_number,
            Kind::Constant => palette.syntax_constant,
            Kind::Operator => palette.syntax_operator,
            Kind::Keyword => palette.syntax_keyword,
        };
        highlighter::Format {
            color: Some(color),
            font: None,
        }
    }
}

/// The Rhai highlighter driven by the text editor via
/// `.highlight_with::<highlight::Highlighter>(...)`.
#[derive(Debug)]
pub struct Highlighter {
    /// Parse-state snapshots, one per [`LINES_PER_SNAPSHOT`] lines.
    caches: Vec<(ParseState, ScopeStack)>,
    current_line: usize,
}

impl highlighter::Highlighter for Highlighter {
    type Settings = ();
    type Highlight = Kind;

    type Iterator<'a> = Box<dyn Iterator<Item = (Range<usize>, Kind)> + 'a>;

    fn new(_settings: &Self::Settings) -> Self {
        Self {
            caches: vec![(ParseState::new(syntax()), ScopeStack::new())],
            current_line: 0,
        }
    }

    fn update(&mut self, _new_settings: &Self::Settings) {
        self.change_line(0);
    }

    fn change_line(&mut self, line: usize) {
        let snapshot = line / LINES_PER_SNAPSHOT;

        if snapshot <= self.caches.len() {
            self.caches.truncate(snapshot);
            self.current_line = snapshot * LINES_PER_SNAPSHOT;
        } else {
            self.caches.truncate(1);
            self.current_line = 0;
        }

        let (parser, stack) = self
            .caches
            .last()
            .cloned()
            .unwrap_or_else(|| (ParseState::new(syntax()), ScopeStack::new()));

        self.caches.push((parser, stack));
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        if self.current_line / LINES_PER_SNAPSHOT >= self.caches.len() {
            let (parser, stack) = self.caches.last().expect("caches are never empty");
            self.caches.push((parser.clone(), stack.clone()));
        }

        self.current_line += 1;

        let (parser, stack) = self.caches.last_mut().expect("caches are never empty");
        let ops = parser.parse_line(line, &SYNTAXES).unwrap_or_default();

        Box::new(kind_ranges(ops, line, stack))
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

/// The embedded Rhai syntax, degrading to plain text if the grammar
/// ever failed to load.
fn syntax() -> &'static SyntaxReference {
    SYNTAXES
        .find_syntax_by_name("Rhai")
        .unwrap_or_else(|| SYNTAXES.find_syntax_plain_text())
}

/// Walks a parsed line's scope operations, yielding every non-plain
/// range with its classified [`Kind`].
fn kind_ranges<'a>(
    ops: Vec<(usize, ScopeStackOp)>,
    line: &str,
    stack: &'a mut ScopeStack,
) -> impl Iterator<Item = (Range<usize>, Kind)> + 'a {
    let ranges = Ranges {
        ops,
        line_length: line.len(),
        index: 0,
        last_index: 0,
    };

    ranges.filter_map(move |(range, op)| {
        let _ = stack.apply(&op);
        if range.is_empty() {
            return None;
        }
        let kind = stack.scopes.iter().rev().find_map(|scope| {
            KINDS
                .iter()
                .find_map(|(prefix, kind)| prefix.is_prefix_of(*scope).then_some(*kind))
        })?;
        Some((range, kind))
    })
}

/// Splits a line into the ranges between scope operations, pairing
/// each range with the operation that begins it (a no-op for the
/// leading range) — the same walk `iced_highlighter` performs.
struct Ranges {
    ops: Vec<(usize, ScopeStackOp)>,
    line_length: usize,
    index: usize,
    last_index: usize,
}

impl Iterator for Ranges {
    type Item = (Range<usize>, ScopeStackOp);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index > self.ops.len() {
            return None;
        }

        let next_index = if self.index == self.ops.len() {
            self.line_length
        } else {
            self.ops[self.index].0
        };

        let range = self.last_index..next_index;
        self.last_index = next_index;

        let op = if self.index == 0 {
            ScopeStackOp::Noop
        } else {
            self.ops[self.index - 1].1.clone()
        };

        self.index += 1;
        Some((range, op))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `lines` in order through a fresh highlighter, returning
    /// each line's classified tokens as (text, kind) pairs.
    fn tokens(lines: &[&str]) -> Vec<Vec<(String, Kind)>> {
        let mut state = <Highlighter as highlighter::Highlighter>::new(&());
        lines
            .iter()
            .map(|line| {
                highlighter::Highlighter::highlight_line(&mut state, line)
                    .map(|(range, kind)| (line[range].to_string(), kind))
                    .collect()
            })
            .collect()
    }

    fn has(tokens: &[(String, Kind)], text: &str, kind: Kind) -> bool {
        tokens
            .iter()
            .any(|(token, k)| token.contains(text) && *k == kind)
    }

    #[test]
    fn the_embedded_grammar_loads() {
        assert!(SYNTAXES.find_syntax_by_name("Rhai").is_some());
    }

    #[test]
    fn keywords_strings_numbers_and_pipes_in_the_starter_are_classified() {
        let lines: Vec<&str> = crate::playback::STARTER.lines().collect();
        let classified = tokens(&lines);

        let all: Vec<(String, Kind)> = classified.into_iter().flatten().collect();
        assert!(has(&all, "fn", Kind::Keyword));
        assert!(has(&all, "let", Kind::Keyword));
        assert!(has(&all, "idle", Kind::String));
        assert!(has(&all, "0", Kind::Number));
        assert!(has(&all, "||", Kind::Operator));
        assert!(has(&all, "did we forget one?", Kind::Comment));
    }

    #[test]
    fn a_line_comment_swallows_the_rest_of_the_line() {
        let classified = tokens(&["e.go_to_floor(2); // let \"x\" = 5"]);
        let line = &classified[0];
        assert!(has(line, "2", Kind::Number));
        // Everything after `//` is one comment token — keywords,
        // quotes, and digits inside it stay comment-colored.
        assert!(has(line, "// let \"x\" = 5", Kind::Comment));
    }

    #[test]
    fn booleans_are_constants_not_keywords() {
        let classified = tokens(&["let armed = true;"]);
        assert!(has(&classified[0], "true", Kind::Constant));
    }

    #[test]
    fn block_comment_state_carries_across_lines() {
        let classified = tokens(&["/* opening", "still inside", "*/ let x = 1;"]);
        assert!(
            classified[1].iter().all(|(_, kind)| *kind == Kind::Comment),
            "the middle line must be entirely comment: {:?}",
            classified[1]
        );
        assert!(!classified[1].is_empty());
        assert!(has(&classified[2], "1", Kind::Number));
    }

    #[test]
    fn string_escapes_stay_string_colored() {
        let classified = tokens(&[r#"let s = "up\ndown";"#]);
        let line = &classified[0];
        assert!(has(line, "up", Kind::String));
        assert!(has(line, r"\n", Kind::String));
    }
}
