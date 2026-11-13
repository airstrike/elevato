//! Rust syntax highlighting for the editor: iced's [`text::Parser`]
//! implemented over syntect's parser, loading Sublime Text's Rust
//! grammar (`assets/rust.sublime-syntax`). Instead of syntect's theme
//! machinery, each parsed scope is classified into a [`Kind`], which
//! [`Kind::style`] colors as the editor draws, so tokens follow the
//! app's light/dark theme without the parser knowing which is active.
//!
//! The colors are the Tokyo City themes' - its dark theme for elevato's
//! dark one, Tokyo City Lighter for the light - through
//! [`grade`](crate::grade::grade), elevato's color grade.
//!
//! The incremental caching (snapshots every [`LINES_PER_SNAPSHOT`]
//! lines, `current_line` tracking) mirrors `iced_highlighter`'s own
//! implementation.

use std::ops::Range;
use std::sync::LazyLock;

use iced::advanced::text::{self, highlighter};
use iced::{Color, Theme};
use syntect::parsing::{
    ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxDefinition, SyntaxReference, SyntaxSet,
    SyntaxSetBuilder,
};

use crate::grade::{adjust, grade};
use crate::theme;

/// Sublime Text's Rust grammar (sublimehq/Packages, under the license in
/// `assets/rust.sublime-syntax.LICENSE`), compiled into the binary so the
/// wasm build needs no asset fetching.
const GRAMMAR: &str = include_str!("../assets/rust.sublime-syntax");

/// Lines per incremental parse snapshot (same rhythm as
/// `iced_highlighter`): editing line N re-parses from the snapshot at
/// the previous multiple of this, not from the top.
const LINES_PER_SNAPSHOT: usize = 50;

/// Built once so edits and redraws do not rebuild the syntax set.
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
/// per stack scope from the innermost scope outwards, so a string's
/// quotes and an attribute's `#` take their construct's kind before the
/// general `punctuation` entry can claim them.
static KINDS: LazyLock<Vec<(Scope, Kind)>> = LazyLock::new(|| {
    let scope = |name: &str| Scope::new(name).expect("invariant: the scope literal parses");
    [
        ("comment", Kind::Comment),
        ("punctuation.definition.comment", Kind::Comment),
        ("constant.character.escape", Kind::Escape),
        ("constant.other.placeholder", Kind::Escape),
        ("string", Kind::String),
        ("punctuation.definition.string", Kind::String),
        ("constant", Kind::Constant),
        ("entity.name.constant", Kind::Constant),
        ("keyword.declaration.import", Kind::Import),
        ("keyword.operator", Kind::Operator),
        ("keyword", Kind::Keyword),
        ("storage.modifier", Kind::Modifier),
        ("storage.type", Kind::Type),
        ("support.type", Kind::Type),
        ("entity.name.struct", Kind::Type),
        ("entity.name.enum", Kind::Type),
        ("entity.name.union", Kind::Type),
        ("entity.name.trait", Kind::Type),
        ("entity.name.type", Kind::Type),
        ("entity.name.impl", Kind::Type),
        ("support.macro", Kind::Macro),
        ("variable.annotation", Kind::Attribute),
        ("punctuation.definition.annotation", Kind::Attribute),
        ("entity.name.function", Kind::Function),
        ("variable.function", Kind::Function),
        ("variable.language", Kind::SelfValue),
        ("variable.parameter", Kind::Parameter),
        ("variable.other.member", Kind::Field),
        ("punctuation", Kind::Punctuation),
    ]
    .into_iter()
    .map(|(name, kind)| (scope(name), kind))
    .collect()
});

/// What a highlighted token is. [`Kind::style`] colors each kind; plain
/// identifiers stay the editor's text color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Line and block comments.
    Comment,
    /// String and character literals.
    String,
    /// Escapes and format placeholders inside a string.
    Escape,
    /// Numbers, `true` and `false`, and named constants.
    Constant,
    /// Control and declaration keywords.
    Keyword,
    /// `use`.
    Import,
    /// `pub`, `mut` and lifetimes.
    Modifier,
    /// Types: primitives, standard and declared ones, and traits.
    Type,
    /// Function and method names, declared or called.
    Function,
    /// Macro invocations.
    Macro,
    /// Attributes: `#[derive(...)]` and kin.
    Attribute,
    /// Function and closure parameters.
    Parameter,
    /// Struct fields.
    Field,
    /// `self`.
    SelfValue,
    /// Operators, `as` and `?`.
    Operator,
    /// Brackets, separators and path accessors.
    Punctuation,
}

impl Kind {
    /// The style of this kind under `theme`: a foreground color only;
    /// the font stays the editor's monospace.
    pub fn style(self, theme: &Theme) -> highlighter::Style {
        let colors = if theme::is_dark(theme) {
            &*DARK
        } else {
            &*LIGHT
        };
        highlighter::Style {
            color: Some(colors[self as usize]),
            style: None,
        }
    }
}

/// The colors of every [`Kind`], in declaration order.
type Colors = [Color; 16];

/// A kind's step off the graded palette: OKLab lightness added, a chroma
/// multiplier, and degrees of hue turned.
type Step = (Kind, f32, f32, f32);

/// Tokyo City's colors for every kind, graded with `chroma` and stepped.
fn graded(kinds: [u32; 16], chroma: f32, steps: &[Step]) -> Colors {
    let mut colors = kinds.map(|hex| grade(Color::from_packed_rgb8(hex), chroma));
    for &(kind, lightness, chroma, hue) in steps {
        colors[kind as usize] = adjust(colors[kind as usize], lightness, chroma, hue);
    }
    colors
}

/// Tokyo City's dark theme, graded. elevato's graphite is near-neutral
/// where Tokyo City's slate carries its blue, so the palette takes less
/// chroma, and its pale symbols, keywords and warm accents step quieter
/// still.
static DARK: LazyLock<Colors> = LazyLock::new(|| {
    graded(
        [
            0x526270, // Comment
            0x9ece6a, // String
            0x89ddff, // Escape
            0xff9e64, // Constant
            0xbb9af7, // Keyword
            0x7dcfff, // Import
            0x9d7cd8, // Modifier
            0x0db9d7, // Type: support.type
            0x7aa2f7, // Function
            0x73daca, // Macro: meta.preprocessor
            0x73daca, // Attribute: meta.preprocessor
            0xe0af68, // Parameter
            0x7dcfff, // Field: variable.other.property
            0xf7768e, // SelfValue: variable.language
            0x89ddff, // Operator
            0x9abdf5, // Punctuation: meta.brace
        ],
        0.84,
        &[
            (Kind::Comment, 0.0, 0.55, -50.0),
            (Kind::Macro, 0.0, 0.8, 6.0),
            (Kind::Attribute, 0.0, 0.8, 6.0),
            (Kind::Keyword, 0.0, 0.74, 0.0),
            (Kind::Modifier, 0.0, 0.74, 0.0),
            (Kind::Import, 0.0, 0.75, 0.0),
            (Kind::Field, 0.0, 0.75, 0.0),
            (Kind::Operator, 0.0, 0.75, 0.0),
            (Kind::Escape, 0.0, 0.75, 0.0),
            (Kind::Punctuation, 0.0, 0.75, 0.0),
            (Kind::String, 0.0, 0.86, 0.0),
            (Kind::Constant, 0.0, 0.86, 0.0),
            (Kind::SelfValue, 0.0, 0.82, 8.0),
        ],
    )
});

/// Tokyo City Lighter, graded. Functions step lighter than the fields
/// they would otherwise match, as Tokyo City Lighter's blue does, and
/// comments warm to the paper and step darker to stay legible on it.
static LIGHT: LazyLock<Colors> = LazyLock::new(|| {
    graded(
        [
            0x9699a3, // Comment
            0x485e30, // String
            0x4c505e, // Escape
            0x965027, // Constant
            0x5a4a78, // Keyword
            0x0f4b6e, // Import
            0x565f89, // Modifier
            0x166775, // Type: support.type
            0x34548a, // Function
            0x33635c, // Macro: meta.preprocessor
            0x33635c, // Attribute: meta.preprocessor
            0x8f5e15, // Parameter
            0x0f4b6e, // Field: variable.other.property
            0x8c4351, // SelfValue: variable.language
            0x4c505e, // Operator
            0x484c61, // Punctuation: meta.brace
        ],
        0.98,
        &[
            (Kind::Comment, 0.0, 0.55, -50.0),
            (Kind::Macro, 0.0, 0.8, 6.0),
            (Kind::Attribute, 0.0, 0.8, 6.0),
            (Kind::Function, 0.02, 0.95, 0.0),
            (Kind::Comment, -0.04, 1.0, 0.0),
        ],
    )
});

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

/// Uses plain text if the embedded Rust grammar fails to load.
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

    /// Feeds `lines` in order through a fresh parser, returning each
    /// line's classified tokens as (text, kind) pairs.
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

    /// The kind of the token that is exactly `text`, if one is.
    fn kind(tokens: &[(String, Kind)], text: &str) -> Option<Kind> {
        tokens
            .iter()
            .find(|(token, _)| token == text)
            .map(|&(_, kind)| kind)
    }

    /// The single line `line`, classified.
    fn line(line: &str) -> Vec<(String, Kind)> {
        tokens(&[line]).remove(0)
    }

    #[test]
    fn the_embedded_grammar_loads() {
        assert!(SYNTAXES.find_syntax_by_name("Rust").is_some());
    }

    #[test]
    fn the_starter_reads_as_rust() {
        let lines: Vec<&str> = crate::playback::STARTER.lines().collect();
        let all: Vec<(String, Kind)> = tokens(&lines).into_iter().flatten().collect();
        assert_eq!(kind(&all, "use"), Some(Kind::Import));
        assert_eq!(kind(&all, "fn"), Some(Kind::Keyword));
        assert_eq!(kind(&all, "match"), Some(Kind::Keyword));
        assert_eq!(kind(&all, "pub"), Some(Kind::Modifier));
        assert_eq!(kind(&all, "Message"), Some(Kind::Type));
        assert_eq!(kind(&all, "go_to_floor"), Some(Kind::Function));
        assert_eq!(kind(&all, "self"), Some(Kind::Parameter));
        assert_eq!(kind(&all, "0"), Some(Kind::Constant));
        assert_eq!(kind(&all, "=>"), Some(Kind::Operator));
        assert_eq!(kind(&all, "::"), Some(Kind::Punctuation));
    }

    #[test]
    fn types_primitives_and_suffixes_are_types() {
        let tokens = line("let x: Vec<usize> = 1_000u32 as usize;");
        assert_eq!(kind(&tokens, "Vec"), Some(Kind::Type));
        assert_eq!(kind(&tokens, "usize"), Some(Kind::Type));
        assert_eq!(kind(&tokens, "1_000"), Some(Kind::Constant));
        assert_eq!(kind(&tokens, "u32"), Some(Kind::Type));
        assert_eq!(kind(&tokens, "as"), Some(Kind::Operator));
    }

    #[test]
    fn declarations_and_calls_name_functions() {
        let tokens = line("pub fn go(&mut self, n: u8) { self.stops.first(); }");
        assert_eq!(kind(&tokens, "go"), Some(Kind::Function));
        assert_eq!(kind(&tokens, "first"), Some(Kind::Function));
        assert_eq!(kind(&tokens, "n"), Some(Kind::Parameter));
        assert_eq!(kind(&tokens, "mut"), Some(Kind::Modifier));
    }

    #[test]
    fn macros_and_attributes_are_their_own_kinds() {
        let tokens = line(r#"println!("{x}");"#);
        assert_eq!(kind(&tokens, "println!"), Some(Kind::Macro));
        assert_eq!(kind(&tokens, "{x}"), Some(Kind::Escape));
        let tokens = line("#[derive(Debug, Clone)]");
        assert_eq!(kind(&tokens, "#"), Some(Kind::Attribute));
        assert_eq!(kind(&tokens, "derive"), Some(Kind::Attribute));
    }

    #[test]
    fn a_line_comment_swallows_the_rest_of_the_line() {
        let tokens = line(r#"go_to_floor(e, 2); // let "x" = 5"#);
        assert_eq!(kind(&tokens, "2"), Some(Kind::Constant));
        let comment = tokens
            .iter()
            .filter(|&&(_, kind)| kind == Kind::Comment)
            .map(|(token, _)| token.as_str())
            .collect::<String>();
        assert_eq!(comment, r#"// let "x" = 5"#);
    }

    #[test]
    fn booleans_are_constants_not_keywords() {
        assert_eq!(
            kind(&line("let armed = true;"), "true"),
            Some(Kind::Constant)
        );
    }

    #[test]
    fn block_comment_state_carries_across_lines() {
        let classified = tokens(&["/* opening", "still inside", "*/ let x = 1;"]);
        assert!(!classified[1].is_empty());
        assert!(
            classified[1].iter().all(|&(_, kind)| kind == Kind::Comment),
            "the middle line is all comment: {:?}",
            classified[1]
        );
        assert_eq!(kind(&classified[2], "1"), Some(Kind::Constant));
    }

    #[test]
    fn nested_block_comments_close_at_their_own_end() {
        let tokens = line("/* outer /* inner */ still */ let x = 1;");
        assert_eq!(kind(&tokens, "let"), Some(Kind::Keyword));
        assert!(
            !tokens
                .iter()
                .any(|(token, kind)| token.contains("still") && *kind != Kind::Comment),
            "the outer comment runs past the inner one's end: {tokens:?}"
        );
    }

    #[test]
    fn escapes_stand_out_inside_strings() {
        let tokens = line(r#"let s = "up\ndown";"#);
        assert_eq!(kind(&tokens, r"\n"), Some(Kind::Escape));
        assert!(
            tokens
                .iter()
                .any(|(token, kind)| token.contains("up") && *kind == Kind::String)
        );
    }

    #[test]
    fn raw_strings_run_to_their_matching_hashes() {
        let tokens = line(r###"let s = r#"say "hi""#; let n = 3;"###);
        assert!(
            tokens
                .iter()
                .any(|(token, kind)| token.contains("hi") && *kind == Kind::String)
        );
        // The inner quote does not close the raw string; the `"#` does.
        assert_eq!(kind(&tokens, "3"), Some(Kind::Constant));
    }

    #[test]
    fn lifetimes_are_modifiers_and_chars_are_strings() {
        let tokens = line("fn f<'a>(x: &'a str) -> char { 'z' }");
        assert_eq!(kind(&tokens, "'a"), Some(Kind::Modifier));
        assert_eq!(kind(&tokens, "z"), Some(Kind::String));
    }

    #[test]
    fn self_in_a_body_is_self() {
        let tokens = line("let n = self.count;");
        assert_eq!(kind(&tokens, "self"), Some(Kind::SelfValue));
    }

    #[test]
    fn every_kind_has_a_color_in_both_themes() {
        for kind in [Kind::Comment, Kind::Punctuation] {
            for theme in [Theme::Light, Theme::Dark] {
                assert!(kind.style(&theme).color.is_some());
            }
        }
    }
}
