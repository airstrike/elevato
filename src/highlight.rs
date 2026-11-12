//! Rust syntax highlighting for the editor: iced's [`text::Parser`]
//! implemented over syntect's parser, loading the embedded
//! `assets/rust.sublime-syntax` grammar. Instead of syntect's theme
//! machinery, each parsed scope is classified into a [`Kind`], which
//! [`Kind::style`] colors from [`crate::theme`]'s palette as the editor
//! draws, so tokens follow the app's light/dark theme without the parser
//! knowing which theme is active.
//!
//! The incremental caching (snapshots every [`LINES_PER_SNAPSHOT`]
//! lines, `current_line` tracking) mirrors `iced_highlighter`'s own
//! implementation.

use std::ops::Range;
use std::sync::LazyLock;

use iced::Theme;
use iced::advanced::text::{self, highlighter};
use syntect::parsing::{
    ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxDefinition, SyntaxReference, SyntaxSet,
    SyntaxSetBuilder,
};

use crate::theme;

/// The embedded Rust grammar, compiled into the binary so the wasm
/// build needs no asset fetching.
const GRAMMAR: &str = include_str!("../assets/rust.sublime-syntax");

/// Lines per incremental parse snapshot (same rhythm as
/// `iced_highlighter`): editing line N re-parses from the snapshot at
/// the previous multiple of this, not from the top.
const LINES_PER_SNAPSHOT: usize = 50;

/// The syntax set, built exactly once - syntax-set construction is far
/// too expensive for anything per-frame or per-keystroke.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(|| {
    let mut builder = SyntaxSetBuilder::new();
    builder.add_plain_text_syntax();
    // `false`: the editor hands lines over without trailing newlines.
    // A malformed grammar degrades to plain text rather than panicking;
    // the tests below pin that it actually loads.
    if let Ok(definition) = SyntaxDefinition::load_from_str(GRAMMAR, false, Some("Rust")) {
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

/// What a highlighted token is. [`Kind::style`] colors each kind from
/// the current theme's palette.
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
    /// The style of this kind under `theme`: a foreground color only;
    /// the font stays the editor's monospace.
    pub fn style(self, theme: &Theme) -> highlighter::Style {
        let palette = theme::palette(theme);
        let color = match self {
            Kind::Comment => palette.syntax_comment,
            Kind::String => palette.syntax_string,
            Kind::Number => palette.syntax_number,
            Kind::Constant => palette.syntax_constant,
            Kind::Operator => palette.syntax_operator,
            Kind::Keyword => palette.syntax_keyword,
        };
        highlighter::Style {
            color: Some(color),
            style: None,
        }
    }
}

/// The Rust parser the text editor highlights with, through
/// `.highlight_with::<highlight::Parser>((), Kind::style)`.
#[derive(Debug)]
pub struct Parser {
    /// Parse-state snapshots, one per [`LINES_PER_SNAPSHOT`] lines.
    caches: Vec<(ParseState, ScopeStack)>,
    current_line: usize,
}

impl text::Parser for Parser {
    type Settings = ();
    type Output = Kind;

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

    fn parse_line(&mut self, line: &str) -> Self::Iterator<'_> {
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

/// The embedded Rust syntax, degrading to plain text if the grammar
/// ever failed to load.
fn syntax() -> &'static SyntaxReference {
    SYNTAXES
        .find_syntax_by_name("Rust")
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
/// leading range) - the same walk `iced_highlighter` performs.
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
        let mut state = <Parser as text::Parser>::new(&());
        lines
            .iter()
            .map(|line| {
                text::Parser::parse_line(&mut state, line)
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
        assert!(SYNTAXES.find_syntax_by_name("Rust").is_some());
    }

    #[test]
    fn keywords_types_numbers_and_arrows_in_the_starter_are_classified() {
        let lines: Vec<&str> = crate::playback::STARTER.lines().collect();
        let classified = tokens(&lines);

        let all: Vec<(String, Kind)> = classified.into_iter().flatten().collect();
        assert!(has(&all, "fn", Kind::Keyword));
        assert!(has(&all, "match", Kind::Keyword));
        assert!(has(&all, "struct", Kind::Keyword));
        // CamelCase reads as a type; the grammar files it under the
        // constant scope.
        assert!(has(&all, "Message", Kind::Constant));
        assert!(has(&all, "0", Kind::Number));
        assert!(has(&all, "=>", Kind::Operator));
        assert!(has(&all, "did we forget one?", Kind::Comment));
    }

    #[test]
    fn a_line_comment_swallows_the_rest_of_the_line() {
        let classified = tokens(&["go_to_floor(e, 2); // let \"x\" = 5"]);
        let line = &classified[0];
        assert!(has(line, "2", Kind::Number));
        // Everything after `//` is one comment token - keywords,
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

    #[test]
    fn raw_strings_run_to_their_matching_hashes() {
        let classified = tokens(&[r###"let s = r#"say "hi""#; let n = 3;"###]);
        let line = &classified[0];
        assert!(has(line, "say", Kind::String));
        // The inner quote does not close the raw string...
        assert!(has(line, "hi", Kind::String));
        // ...and the closing `"#` does: what follows is code again.
        assert!(has(line, "3", Kind::Number));
    }

    #[test]
    fn suffixed_numbers_are_numbers_and_lifetimes_are_not_chars() {
        let classified = tokens(&["fn f<'a>(x: &'a str) -> char { let n = 2usize; 'z' }"]);
        let line = &classified[0];
        assert!(has(line, "2usize", Kind::Number));
        assert!(has(line, "'z'", Kind::String));
        assert!(
            !line
                .iter()
                .any(|(token, kind)| token.contains("'a") && *kind == Kind::String),
            "a lifetime is not a char literal: {line:?}"
        );
    }
}
