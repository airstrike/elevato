//! Cmd+click lookup for documented API names. Imports and qualified paths lead
//! to definitions on the library's pages; method calls require a unique
//! documented name because this lookup does not infer types.
//!
//! A click takes the path that ends at the clicked name - `Message`,
//! `elevator::Message::Idle`, `Elevator::new` - and resolves its first segment
//! in the clicked file's scope:
//!
//! - In the player's `main.rs`: the names its `use elevato::…` items import,
//!   and `elevato` itself. A name the file defines shadows an import, and
//!   resolves only when it is `Program`, whose functions the library documents.
//! - On a page: the page's own items, then the crate root's, and `elevato`.
//!
//! A module counts only before `::`, as in Rust's namespaces: a variable named
//! `elevator` never opens `elevator.rs`. Each later segment walks into a
//! module, then an item, then its variant or method. A method call
//! (`.go_to_floor(…)`) has no type to walk from, so it resolves only when
//! exactly one documented type has that method.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::docs::Page;

/// Where a click's path is resolved.
#[derive(Debug, Clone, Copy)]
pub enum Scope<'a> {
    /// The player's program, with its whole text: its imports and its own
    /// definitions decide what a name means.
    Program(&'a str),
    /// A page of the library.
    Page(Page),
}

/// The definition the click at byte `index` of line `line` in `scope` names:
/// its page and line.
pub fn resolve(scope: Scope<'_>, line: usize, index: usize) -> Option<(Page, usize)> {
    let source = match scope {
        Scope::Program(text) => text,
        Scope::Page(page) => page.source(),
    };
    let text = source.lines().nth(line)?;
    let column = text.get(..index)?.chars().count();
    let clicked = Clicked::at(text, column)?;

    if clicked.method {
        return INDEX.method(&clicked.path[0]);
    }
    let mut path = clicked.path;
    if let Scope::Program(source) = scope
        && path.len() == 1
        && let Some(owner) = defining_impl(source, line, &path[0])
    {
        path.insert(0, owner);
    }

    let mut target = match scope {
        Scope::Program(source) => Program::new(source).first(&path, clicked.before_path)?,
        Scope::Page(page) => first_on_page(page, &path[0], path.len() > 1 || clicked.before_path)?,
    };
    for segment in &path[1..] {
        target = target.walk(segment)?;
    }
    target.site()
}

/// The identifier under `column` in `line`: the word a click landed in, or the
/// one it landed just after.
pub fn identifier_at(line: &str, column: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    let (start, end) = word_around(&chars, column)?;
    Some(chars[start..end].iter().collect())
}

/// A click's path: its segments up to the clicked one, whether a `::` follows
/// the clicked one, and whether it is a method call.
struct Clicked {
    path: Vec<String>,
    before_path: bool,
    method: bool,
}

impl Clicked {
    fn at(line: &str, column: usize) -> Option<Self> {
        let chars: Vec<char> = line.chars().collect();
        let (start, end) = word_around(&chars, column)?;
        let mut path = vec![chars[start..end].iter().collect::<String>()];

        let mut at = start;
        while at >= 2 && chars[at - 1] == ':' && chars[at - 2] == ':' {
            let end = at - 2;
            let begin = (0..end).rev().take_while(|&i| is_word(chars[i])).last()?;
            path.insert(0, chars[begin..end].iter().collect());
            at = begin;
        }
        let method = path.len() == 1
            && chars[..start]
                .iter()
                .rev()
                .find(|c| !c.is_whitespace())
                .is_some_and(|&c| c == '.');
        let before_path = chars.get(end) == Some(&':') && chars.get(end + 1) == Some(&':');
        Some(Self {
            path,
            before_path,
            method,
        })
    }
}

/// What a path's segments have resolved to so far.
#[derive(Debug, Clone)]
enum Target {
    /// The crate, `elevato`.
    Root,
    /// One of its modules: a page.
    Module(Page),
    /// An item defined on a page.
    Item(Page, String),
    /// A variant, method or field, at its line.
    Member(Page, usize),
}

impl Target {
    fn walk(self, segment: &str) -> Option<Self> {
        match self {
            Self::Root => module(segment).map(Self::Module).or_else(|| root(segment)),
            Self::Module(page) => INDEX
                .items
                .contains_key(&(page, segment.to_owned()))
                .then(|| Self::Item(page, segment.to_owned())),
            Self::Item(page, item) => INDEX
                .members
                .get(&(page, item, segment.to_owned()))
                .map(|&line| Self::Member(page, line)),
            Self::Member(..) => None,
        }
    }

    fn site(self) -> Option<(Page, usize)> {
        match self {
            Self::Root => None,
            Self::Module(page) => Some((page, 0)),
            Self::Item(page, item) => INDEX.items.get(&(page, item)).map(|&line| (page, line)),
            Self::Member(page, line) => Some((page, line)),
        }
    }
}

/// The player's program, as far as resolution reads it.
struct Program {
    /// Each imported name, and the path it names.
    imports: HashMap<String, Vec<String>>,
    /// The names the program defines itself.
    locals: Vec<String>,
}

impl Program {
    fn new(source: &str) -> Self {
        Self {
            imports: imports(source),
            locals: locals(source),
        }
    }

    fn first(&self, path: &[String], before_path: bool) -> Option<Target> {
        let name = path[0].as_str();
        let modules = path.len() > 1 || before_path;
        if name == "elevato" {
            return Some(Target::Root);
        }
        if self.locals.iter().any(|local| local == name) {
            return (name == "Program").then(|| Target::Item(Page::Lib, name.to_owned()));
        }
        let imported = self.imports.get(name)?;
        let mut target = Target::Root;
        for segment in &imported[1..] {
            target = target.walk(segment)?;
        }
        match target {
            Target::Module(_) if !modules => None,
            target => Some(target),
        }
    }
}

/// The first segment of a path on `page`.
fn first_on_page(page: Page, name: &str, modules: bool) -> Option<Target> {
    if name == "elevato" {
        return Some(Target::Root);
    }
    if INDEX.items.contains_key(&(page, name.to_owned())) {
        return Some(Target::Item(page, name.to_owned()));
    }
    if modules && let Some(page) = module(name) {
        return Some(Target::Module(page));
    }
    root(name)
}

/// The crate's modules, one per page.
fn module(name: &str) -> Option<Page> {
    match name {
        "building" => Some(Page::Building),
        "elevator" => Some(Page::Elevator),
        "floor" => Some(Page::Floor),
        _ => None,
    }
}

/// An item of the crate root: `lib.rs`'s own, or a type it re-exports.
fn root(name: &str) -> Option<Target> {
    let page = match name {
        "Building" => Page::Building,
        "Elevator" => Page::Elevator,
        _ => Page::Lib,
    };
    INDEX
        .items
        .contains_key(&(page, name.to_owned()))
        .then(|| Target::Item(page, name.to_owned()))
}

/// The library's definitions, read from its pages.
struct Index {
    /// `(page, item)` → its line.
    items: HashMap<(Page, String), usize>,
    /// `(page, item, member)` → its line.
    members: HashMap<(Page, String, String), usize>,
    /// Method name → every `(page, line)` defining it.
    methods: HashMap<String, Vec<(Page, usize)>>,
}

static INDEX: LazyLock<Index> = LazyLock::new(|| {
    let mut index = Index {
        items: HashMap::new(),
        members: HashMap::new(),
        methods: HashMap::new(),
    };
    for &page in Page::ALL {
        let mut owner: Option<(String, bool)> = None;
        for (line, text) in page.source().lines().enumerate() {
            if text.starts_with('}') {
                owner = None;
                continue;
            }
            if !text.starts_with(' ') {
                let Some((keyword, name)) = declaration(text) else {
                    continue;
                };
                index.items.entry((page, name.clone())).or_insert(line);
                owner = text
                    .trim_end()
                    .ends_with('{')
                    .then_some((name, keyword == "impl"));
                continue;
            }
            let Some((name, implementation)) = &owner else {
                continue;
            };
            let Some(member) = member(text) else {
                continue;
            };
            index
                .members
                .entry((page, name.clone(), member.clone()))
                .or_insert(line);
            if *implementation {
                index.methods.entry(member).or_default().push((page, line));
            }
        }
    }
    index
});

impl Index {
    fn method(&self, name: &str) -> Option<(Page, usize)> {
        match self.methods.get(name)?.as_slice() {
            [site] => Some(*site),
            _ => None,
        }
    }
}

/// A top-level line's keyword and name: `pub enum X`, `pub struct X`, `pub fn
/// x`, `impl X`.
fn declaration(line: &str) -> Option<(&'static str, String)> {
    let line = line.strip_prefix("pub ").unwrap_or(line);
    ["enum", "struct", "fn", "impl", "type", "const"]
        .into_iter()
        .find_map(|keyword| {
            let rest = line.strip_prefix(keyword)?.strip_prefix(' ')?;
            Some((keyword, leading_word(rest)?))
        })
}

/// A member line one level into a block: `pub fn x`, `pub x: T`, or an enum's
/// `Variant`. Deeper lines - a variant's fields - are not members.
fn member(line: &str) -> Option<String> {
    let inner = line.strip_prefix("    ")?;
    if inner.starts_with(' ') || inner.starts_with("//") {
        return None;
    }
    let inner = inner.strip_prefix("pub ").unwrap_or(inner);
    let inner = inner.strip_prefix("fn ").unwrap_or(inner);
    leading_word(inner)
}

/// The names `use elevato::…` items bring into scope, with their paths.
fn imports(source: &str) -> HashMap<String, Vec<String>> {
    let mut imports = HashMap::new();
    let mut rest = source;
    while let Some(start) = rest.find("use ") {
        let after = &rest[start + 4..];
        let Some(end) = after.find(';') else { break };
        let tree: String = after[..end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        rest = &after[end..];

        let (prefix, names) = match tree.split_once("::{") {
            Some((prefix, names)) => (prefix, names.trim_end_matches('}').to_owned()),
            None => match tree.rsplit_once("::") {
                Some((prefix, name)) => (prefix, name.to_owned()),
                None => continue,
            },
        };
        let prefix: Vec<String> = prefix.split("::").map(str::to_owned).collect();
        if prefix.first().map(String::as_str) != Some("elevato") {
            continue;
        }
        for name in names
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            let (original, alias) = name.split_once(" as ").unwrap_or((name, name));
            let mut path = prefix.clone();
            if original != "self" {
                path.push(original.to_owned());
            }
            let alias = if alias == "self" {
                prefix.last().cloned().unwrap_or_default()
            } else {
                alias.to_owned()
            };
            imports.insert(alias, path);
        }
    }
    imports
}

/// The names the program defines at its top level.
fn locals(source: &str) -> Vec<String> {
    source
        .lines()
        .filter(|line| !line.starts_with(' '))
        .filter_map(declaration)
        .filter(|(keyword, _)| *keyword != "impl")
        .map(|(_, name)| name)
        .collect()
}

/// The type whose `impl` block defines `name` at line `line`, when that line is
/// its `fn name` - a click on a function's own definition.
fn defining_impl(source: &str, line: usize, name: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let text = lines.get(line)?.trim_start();
    let text = text.strip_prefix("pub ").unwrap_or(text);
    if leading_word(text.strip_prefix("fn ")?)? != name {
        return None;
    }
    lines[..line].iter().rev().find_map(|line| {
        if line.starts_with('}') {
            return Some(None);
        }
        let ("impl", owner) = declaration(line)? else {
            return None;
        };
        Some(Some(owner))
    })?
}

/// The bounds of the word around `column`: the one it is in, or the one it is
/// just past.
fn word_around(chars: &[char], column: usize) -> Option<(usize, usize)> {
    let at = if chars.get(column).is_some_and(|&c| is_word(c)) {
        column
    } else {
        column.checked_sub(1).filter(|&at| is_word(chars[at]))?
    };
    let start = (0..=at).rev().take_while(|&i| is_word(chars[i])).last()?;
    let end = (at..chars.len())
        .take_while(|&i| is_word(chars[i]))
        .last()?
        + 1;
    (!chars[start].is_ascii_digit()).then_some((start, end))
}

/// The identifier a string starts with.
fn leading_word(text: &str) -> Option<String> {
    let end = text.find(|c: char| !is_word(c)).unwrap_or(text.len());
    (end > 0).then(|| text[..end].to_owned())
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTER: &str = include_str!("../tests/solutions/starter.rs");

    /// Where a click on `word`, in the starter's first line containing
    /// `context`, lands; `word` is found within `context`.
    fn click(context: &str, word: &str) -> Option<(Page, usize)> {
        let line = STARTER
            .lines()
            .position(|line| line.contains(context))
            .unwrap_or_else(|| panic!("the starter has {context:?}"));
        let text = STARTER.lines().nth(line).unwrap();
        let column = text.find(context).unwrap()
            + context
                .find(word)
                .unwrap_or_else(|| panic!("{word:?} is in {context:?}"));
        resolve(Scope::Program(STARTER), line, column)
    }

    /// The text of the line a site points at.
    fn line_of((page, line): (Page, usize)) -> &'static str {
        page.source()
            .lines()
            .nth(line)
            .expect("the site is on its page")
    }

    #[test]
    fn the_programs_message_is_the_library_root_message() {
        let site = click("message: Message", "Message").expect("Message resolves");
        assert_eq!(site.0, Page::Lib);
        assert_eq!(line_of(site), "pub enum Message {");
    }

    #[test]
    fn a_qualified_path_walks_module_item_and_variant() {
        let site = click("elevator::Message::Idle", "Idle").expect("the variant resolves");
        assert_eq!(site.0, Page::Elevator);
        assert!(line_of(site).trim_start().starts_with("Idle"));

        let site = click("elevator::Message::Idle", "Message").expect("elevator::Message resolves");
        assert_eq!(site.0, Page::Elevator);
        assert_eq!(line_of(site), "pub enum Message {");
    }

    #[test]
    fn a_variant_of_the_root_message_is_its_own_line() {
        let site = click("Message::Elevator(", "Elevator").expect("the variant resolves");
        assert_eq!(site.0, Page::Lib);
        assert!(line_of(site).trim_start().starts_with("Elevator("));
    }

    #[test]
    fn a_variable_named_like_a_module_resolves_to_nothing() {
        // `building.floor_of(elevator)`: `elevator` the car, not the module.
        let line = STARTER
            .lines()
            .position(|line| line.contains("floor_of(elevator)"))
            .expect("the starter reads the car's floor");
        let column = STARTER
            .lines()
            .nth(line)
            .unwrap()
            .find("(elevator")
            .unwrap()
            + 1;
        assert_eq!(resolve(Scope::Program(STARTER), line, column), None);
    }

    #[test]
    fn a_unique_method_resolves_through_its_call() {
        let site = click(".go_to_floor", "go_to_floor").expect(".go_to_floor resolves");
        assert_eq!(site.0, Page::Elevator);
        assert!(line_of(site).contains("pub fn go_to_floor"));
    }

    #[test]
    fn the_programs_own_functions_resolve_to_the_documented_program() {
        let site = click("pub fn update", "update").expect("pub fn update resolves");
        assert_eq!(site.0, Page::Lib);
        assert!(line_of(site).contains("pub fn update"));
        let site = click("Program::update", "update").expect("Program::update resolves");
        assert!(line_of(site).contains("pub fn update"));
    }

    #[test]
    fn on_a_page_a_bare_name_is_the_pages_own_before_the_roots() {
        let line = Page::Elevator
            .source()
            .lines()
            .position(|line| line.contains("pub fn new()"))
            .unwrap();
        let text = Page::Elevator.source().lines().nth(line).unwrap();
        let own = text.find("Elevator,").unwrap();
        let root = text.find("elevato::Message").unwrap() + "elevato::".len();
        let site = resolve(Scope::Page(Page::Elevator), line, own).unwrap();
        assert_eq!(line_of(site), "pub struct Elevator;");
        let site = resolve(Scope::Page(Page::Elevator), line, root).unwrap();
        assert_eq!((site.0, line_of(site)), (Page::Lib, "pub enum Message {"));
    }

    /// Expected reference pages for these API paths. Missing or renamed
    /// declarations fail lookup; new API items need an entry here.
    const CONTRACT: &[(&str, Page)] = &[
        ("elevato::run", Page::Lib),
        ("elevato::view", Page::Lib),
        ("elevato::riders", Page::Lib),
        ("elevato::Direction", Page::Lib),
        ("elevato::Direction::Up", Page::Lib),
        ("elevato::Direction::Down", Page::Lib),
        ("Message", Page::Lib),
        ("Message::Elevator", Page::Lib),
        ("Message::Floor", Page::Lib),
        ("Building", Page::Building),
        ("Building::floors", Page::Building),
        ("Building::floor_of", Page::Building),
        ("Elevator", Page::Elevator),
        ("Elevator::new", Page::Elevator),
        ("Elevator::default", Page::Elevator),
        ("Elevator::go_to_floor", Page::Elevator),
        ("Elevator::stop", Page::Elevator),
        ("Elevator::set_going_up_indicator", Page::Elevator),
        ("Elevator::set_going_down_indicator", Page::Elevator),
        ("elevator::Message", Page::Elevator),
        ("elevator::Message::Idle", Page::Elevator),
        ("elevator::Message::FloorButtonPressed", Page::Elevator),
        ("elevator::Message::PassingFloor", Page::Elevator),
        ("elevator::Message::StoppedAtFloor", Page::Elevator),
        ("elevato::floor::Message", Page::Floor),
        ("elevato::floor::Message::UpButtonPressed", Page::Floor),
        ("elevato::floor::Message::DownButtonPressed", Page::Floor),
    ];

    #[test]
    fn every_path_in_the_api_contract_resolves_to_its_page() {
        let misplaced: Vec<_> = CONTRACT
            .iter()
            .filter(|&&(path, page)| {
                let source = format!(
                    "{}\n{path}",
                    STARTER
                        .lines()
                        .find(|line| line.starts_with("use elevato"))
                        .unwrap()
                );
                resolve(Scope::Program(&source), 1, path.len()).map(|(p, _)| p) != Some(page)
            })
            .collect();
        assert!(misplaced.is_empty(), "wrong or missing: {misplaced:?}");
    }

    #[test]
    fn identifiers_resolve_under_and_just_after_the_word() {
        let line = "    pub fn go_to_floor(&self, n: i64);";
        assert_eq!(identifier_at(line, 13).as_deref(), Some("go_to_floor"));
        assert_eq!(identifier_at(line, 22).as_deref(), Some("go_to_floor"));
        assert_eq!(identifier_at(line, 2), None);
    }
}
