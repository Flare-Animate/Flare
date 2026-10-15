//! Frame scripts: a keyframe's ActionScript, as XFL keeps it (`<Actionscript><script>`
//! in the frame), and the small part of the language that plays here: stop, play,
//! gotoAndPlay and gotoAndStop (a frame number or label, and a scene), nextFrame,
//! prevFrame, nextScene, prevScene, trace and navigateToURL / getURL, on the script's
//! own timeline (`this`), its parent, the root or a child instance by name. `parse`
//! reads that subset; anything else stays in the document, is reported and skipped.
//! The SWF export compiles it to AVM1 actions (`to_avm1`), HTML5 Canvas publishing
//! translates it to JavaScript (`to_js`), and Test Movie's player (player.rs) runs it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;

use swf::avm1::types::{Action, GetUrl, GotoFrame, GotoLabel, Push, SetTarget, Value as Pushed};
use swf::avm1::write::Writer;
use swf::{CharacterId, FrameLabel, PlaceObjectAction, SwfStr, Tag};

use crate::display::{Item, Placed, display_list};
use crate::doc::{Document, Element, Frame, LayerKind, Symbol, Timeline};

// ---- The script in the document -----------------------------------------------------

impl Frame {
    /// The keyframe's frame script, if it has one.
    pub fn script(&self) -> Option<String> {
        let xml = self.extra.before.iter().chain(&self.extra.after).find(|xml| is_script(xml))?;
        let xml = roxmltree::Document::parse(xml).ok()?;
        let script = xml.root_element().children().find(|n| n.tag_name().name() == "script");
        Some(script.and_then(|n| n.text()).unwrap_or_default().to_string())
    }

    /// Sets the keyframe's frame script; an empty one removes it. It's saved in CDATA, as
    /// Animate saves scripts, and reads back exactly, but for the control characters XML can't
    /// hold at all (a pasted form feed, say), which go as xfl_write's `escape` drops them.
    pub fn set_script(&mut self, text: &str) {
        let text: String = text.chars().filter(|&c| matches!(c, '\t' | '\n' | '\r') || !matches!(c, '\0'..='\u{1f}' | '\u{fffe}' | '\u{ffff}')).collect();
        let text = text.as_str();
        let extra = &mut self.extra;
        if text.is_empty() {
            extra.before.retain(|xml| !is_script(xml));
            extra.after.retain(|xml| !is_script(xml));
            return;
        }
        // Out of the CDATA go `]]>`, which would end it, and carriage returns, which XML reads as line feeds.
        let cdata = text.replace("]]>", "]]]]><![CDATA[>").replace('\r', "]]>&#13;<![CDATA[");
        let xml = format!("<Actionscript><script><![CDATA[{cdata}]]></script></Actionscript>");
        match extra.before.iter_mut().chain(&mut extra.after).find(|x| is_script(x)) {
            Some(slot) => *slot = xml,
            None => extra.before.insert(0, xml),
        }
    }
}

fn is_script(xml: &str) -> bool {
    xml.trim_start().starts_with("<Actionscript")
}

// ---- The subset -------------------------------------------------------------------------

/// The language a document's scripts are in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialect {
    /// ActionScript 3.0, also taking 2.0's spellings (`_parent`, `getURL`): frames count from 1.
    ActionScript,
    /// An HTML5 Canvas document's JavaScript (`this.stop()`): frames count from 0.
    JavaScript,
}

impl Dialect {
    pub fn of(doc: &Document) -> Dialect {
        if doc.is_canvas() { Dialect::JavaScript } else { Dialect::ActionScript }
    }

    /// The number its scripts give a timeline's first frame.
    pub fn first(self) -> u32 {
        match self {
            Dialect::ActionScript => 1,
            Dialect::JavaScript => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub dialect: Dialect,
    pub stmts: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// A command of the subset, on the timeline `path` leads to from the script's own.
    Run { path: Vec<Step>, command: Command },
    /// A statement outside the subset: its line (from 1) and source. It's skipped.
    Skip { line: usize, source: String },
}

/// A step from one timeline to another.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Parent,
    /// The movie's main timeline.
    Root,
    /// The child movie clip with this instance name.
    Child(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Stop,
    Play,
    NextFrame,
    PrevFrame,
    NextScene,
    PrevScene,
    /// gotoAndPlay (`play`) or gotoAndStop.
    Goto { frame: FrameRef, scene: Option<String>, play: bool },
    /// trace: the values, printed with a space between.
    Trace(Vec<Value>),
    /// navigateToURL or getURL: the page, and the window it opens in.
    OpenUrl { url: String, window: String },
}

impl Command {
    /// The command as scripts name it.
    pub fn name(&self) -> &'static str {
        match self {
            Command::Stop => "stop",
            Command::Play => "play",
            Command::NextFrame => "nextFrame",
            Command::PrevFrame => "prevFrame",
            Command::NextScene => "nextScene",
            Command::PrevScene => "prevScene",
            Command::Goto { play: true, .. } => "gotoAndPlay",
            Command::Goto { play: false, .. } => "gotoAndStop",
            Command::Trace(_) => "trace",
            Command::OpenUrl { .. } => "navigateToURL",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FrameRef {
    /// A frame of the scene, from 0 whatever the dialect.
    Number(u32),
    Label(String),
}

/// What trace prints.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Number(f64),
    /// A timeline's currentFrame and totalFrames, as the dialect counts them.
    CurrentFrame(Vec<Step>),
    TotalFrames(Vec<Step>),
    /// `a + b + c`: added as numbers, or joined once one is text.
    Sum(Vec<Value>),
}

/// Reads `src` as far as the subset goes.
pub fn parse(src: &str, dialect: Dialect) -> Script {
    let mut p = Parser { src, toks: tokens(src), dialect, stmts: Vec::new(), aliases: Vec::new() };
    p.statements(0, p.toks.len(), 0);
    Script { dialect, stmts: p.stmts }
}

/// How deep blocks and statements nest before the rest is skipped whole: no script of the
/// subset nests so deep, and a file that does mustn't exhaust the stack.
const DEEPEST: usize = 32;
/// The most tokens a statement of the subset takes; a longer one is skipped unread, for the same reason.
const LONGEST: usize = 256;

/// What a stretch of script source is: the parser's tokens, and what the Actions panel colours.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lex {
    Word,
    Number,
    /// A string literal, quotes included.
    Text,
    Comment,
    Punct,
}

/// The source's tokens and comments with their byte ranges; whitespace is left out.
pub fn lex(src: &str) -> Vec<(Lex, Range<usize>)> {
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let rest = &src[start..];
        let kind = if c.is_whitespace() {
            continue;
        } else if rest.starts_with("//") {
            while chars.next_if(|&(_, d)| d != '\n').is_some() {}
            Lex::Comment
        } else if rest.starts_with("/*") {
            let end = rest[2..].find("*/").map_or(src.len(), |i| start + i + 4);
            while chars.next_if(|&(i, _)| i < end).is_some() {}
            Lex::Comment
        } else if c == '"' || c == '\'' {
            // To the closing quote, past escapes; an unclosed string stops at its line's end.
            while let Some((_, d)) = chars.next_if(|&(_, d)| d != '\n') {
                if d == '\\' {
                    chars.next_if(|&(_, d)| d != '\n');
                } else if d == c {
                    break;
                }
            }
            Lex::Text
        } else if c.is_ascii_digit() || (c == '.' && rest[1..].starts_with(|d: char| d.is_ascii_digit())) {
            let hex = rest.starts_with("0x") || rest.starts_with("0X");
            let mut prev = c;
            while let Some((_, d)) = chars.next_if(|&(_, d)| {
                d.is_ascii_alphanumeric() || d == '.' || (!hex && matches!(d, '+' | '-') && matches!(prev, 'e' | 'E'))
            }) {
                prev = d;
            }
            Lex::Number
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            while chars.next_if(|&(_, d)| d.is_alphanumeric() || d == '_' || d == '$').is_some() {}
            Lex::Word
        } else {
            Lex::Punct
        };
        let end = chars.peek().map_or(src.len(), |&(i, _)| i);
        out.push((kind, start..end));
    }
    out
}

#[derive(Clone, Copy)]
struct Tok<'s> {
    kind: Lex,
    text: &'s str,
    at: usize,
    /// A line break comes between it and the token before.
    nl: bool,
}

const END: Tok<'static> = Tok { kind: Lex::Punct, text: "", at: 0, nl: false };

/// The tokens the parser reads: comments out, but their line breaks still counted.
fn tokens(src: &str) -> Vec<Tok<'_>> {
    let mut out = Vec::new();
    let mut last = 0;
    for (kind, r) in lex(src) {
        if kind != Lex::Comment {
            out.push(Tok { kind, text: &src[r.clone()], at: r.start, nl: src[last..r.start].contains('\n') });
            last = r.end;
        }
    }
    out
}

/// A parsed expression, as far as the subset needs them.
#[derive(Clone, Debug, PartialEq)]
enum Expr<'s> {
    This,
    Name(&'s str),
    Text(String),
    Number(f64),
    Member(Box<Expr<'s>>, &'s str),
    Call(Box<Expr<'s>>, Vec<Expr<'s>>),
    New(&'s str, Vec<Expr<'s>>),
    Add(Box<Expr<'s>>, Box<Expr<'s>>),
}

struct Parser<'s> {
    src: &'s str,
    toks: Vec<Tok<'s>>,
    dialect: Dialect,
    stmts: Vec<Stmt>,
    /// Names the script gives its own timeline (`alias`).
    aliases: Vec<&'s str>,
}

impl<'s> Parser<'s> {
    fn tok(&self, i: usize) -> Tok<'s> {
        self.toks.get(i).copied().unwrap_or(END)
    }

    fn is(&self, i: usize, text: &str) -> bool {
        i < self.toks.len() && self.toks[i].text == text
    }

    /// The statements of tokens `i..end`, `depth` blocks in, run or skipped.
    fn statements(&mut self, mut i: usize, end: usize, depth: usize) {
        while i < end {
            let next = match self.toks[i].text {
                ";" => i + 1,
                // A block runs its statements.
                "{" => {
                    let close = self.close(i);
                    let inner = if self.is(close - 1, "}") && close - 1 > i { close - 1 } else { close };
                    if depth < DEEPEST {
                        self.statements(i + 1, inner.min(end), depth + 1);
                    } else {
                        self.skip(i, close.min(end));
                    }
                    close
                }
                // Imports change nothing the subset does.
                "import" => self.simple_end(i),
                // Conditions, loops and functions aren't run: all of one is skipped, its body too.
                "if" | "else" | "while" | "do" | "for" | "with" | "switch" | "try" | "function" => {
                    let j = self.statement_end(i, 0).max(i + 1);
                    self.skip(i, j);
                    j
                }
                _ => {
                    let j = self.simple_end(i).max(i + 1);
                    let k = if self.is(j - 1, ";") { j - 1 } else { j };
                    if let Some(name) = self.alias(i, k) {
                        self.aliases.push(name);
                    } else {
                        match self.run(i, k) {
                            Some(stmt) => self.stmts.push(stmt),
                            None => self.skip(i, j),
                        }
                    }
                    j
                }
            };
            i = next.max(i + 1);
        }
    }

    /// The index after the bracket that closes the one at `i`.
    fn close(&self, i: usize) -> usize {
        let mut depth = 0;
        for (j, t) in self.toks.iter().enumerate().skip(i) {
            match t.text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => {
                    depth -= 1;
                    if depth == 0 {
                        return j + 1;
                    }
                }
                _ => {}
            }
        }
        self.toks.len()
    }

    /// The index after the statement starting at `i`, whatever kind it is, `depth` statements in;
    /// too deep, the rest of the script.
    fn statement_end(&self, i: usize, depth: usize) -> usize {
        if depth > DEEPEST {
            return self.toks.len();
        }
        let paren = |j: usize| if self.is(j, "(") { self.close(j) } else { j };
        let block = |j: usize| if self.is(j, "{") { self.close(j) } else { j };
        match self.tok(i).text {
            "{" => self.close(i),
            word @ ("if" | "while" | "with" | "for") => {
                let j = if self.is(i + 1, "each") { i + 2 } else { i + 1 };
                let j = self.statement_end(paren(j), depth + 1);
                if word == "if" && self.is(j, "else") { self.statement_end(j + 1, depth + 1) } else { j }
            }
            "switch" => block(paren(i + 1)),
            "do" => {
                let j = self.statement_end(i + 1, depth + 1);
                if self.is(j, "while") { self.simple_end(j) } else { j }
            }
            "try" => {
                let mut j = block(i + 1);
                while self.is(j, "catch") {
                    j = block(paren(j + 1));
                }
                if self.is(j, "finally") { block(j + 1) } else { j }
            }
            "function" => {
                // Its name, parameters and return type, then its body.
                let mut j = i + 1;
                while j < self.toks.len() && !self.is(j, "{") && !self.is(j, ";") {
                    j = paren(j).max(j + 1);
                }
                block(j)
            }
            "else" => self.statement_end(i + 1, depth + 1),
            _ => self.simple_end(i),
        }
    }

    /// The index after a simple statement: past its `;`, or at the line break or `}` it ends at.
    fn simple_end(&self, i: usize) -> usize {
        // What a line can end a statement with: a name, literal or closing bracket, or `++` / `--`.
        let ends = |j: usize| {
            let t = self.toks[j];
            matches!(t.kind, Lex::Word | Lex::Number | Lex::Text)
                || matches!(t.text, ")" | "]" | "}")
                || (matches!(t.text, "+" | "-") && j > 0 && self.toks[j - 1].text == t.text)
        };
        let starts = |t: Tok| matches!(t.kind, Lex::Word | Lex::Number | Lex::Text);
        let mut depth = 0;
        for j in i..self.toks.len() {
            let t = self.toks[j];
            if j > i && depth == 0 && t.nl && ends(j - 1) && starts(t) {
                return j;
            }
            match t.text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" if depth == 0 => return j,
                ")" | "]" | "}" => depth -= 1,
                ";" if depth == 0 => return j + 1,
                _ => {}
            }
        }
        self.toks.len()
    }

    /// Records tokens `i..j` as a statement outside the subset.
    fn skip(&mut self, i: usize, j: usize) {
        let last = self.toks[j - 1];
        let (from, to) = (self.toks[i].at, last.at + last.text.len());
        let line = self.src[..from].matches('\n').count() + 1;
        let mut source = self.src[from..to].split_whitespace().collect::<Vec<_>>().join(" ");
        if source.chars().count() > 80 {
            source = source.chars().take(79).chain(['…']).collect();
        }
        self.stmts.push(Stmt::Skip { line, source });
    }

    /// `var _this = this` (or `let`, `const`, typed or not) in tokens `i..end`, as Animate's HTML5
    /// Canvas snippets name the script's own timeline: the name.
    fn alias(&self, i: usize, end: usize) -> Option<&'s str> {
        if !matches!(end - i, 4 | 6) || self.toks[i + 1].kind != Lex::Word {
            return None;
        }
        let words: Vec<&'s str> = self.toks[i..end].iter().map(|t| t.text).collect();
        match words[..] {
            ["var" | "let" | "const", name, "=", "this"] | ["var" | "let" | "const", name, ":", _, "=", "this"] => Some(name),
            _ => None,
        }
    }

    /// Tokens `i..end` as a command of the subset.
    fn run(&self, i: usize, end: usize) -> Option<Stmt> {
        if end - i > LONGEST {
            return None;
        }
        let mut at = i;
        let Expr::Call(callee, args) = self.expr(&mut at, end)? else { return None };
        if at != end {
            return None;
        }
        // A bare call: stop() rather than this.stop().
        let bare = matches!(*callee, Expr::Name(_));
        let (path, name) = match *callee {
            Expr::Name(name) => (Vec::new(), name),
            // JavaScript's spellings of trace and navigateToURL.
            Expr::Member(obj, "log") if *obj == Expr::Name("console") => (Vec::new(), "trace"),
            Expr::Member(obj, "open") if *obj == Expr::Name("window") => (Vec::new(), "window.open"),
            Expr::Member(obj, name) => (self.path(&obj)?, name),
            _ => return None,
        };
        let command = match (name, &args[..]) {
            ("trace", values) => Command::Trace(values.iter().map(|v| self.value(v)).collect::<Option<_>>()?),
            ("navigateToURL", [Expr::New("URLRequest", url), window @ ..]) if bare => url_command(url, window, "_blank")?,
            ("getURL", [url, window @ ..]) if bare => url_command(std::slice::from_ref(url), window, "_self")?,
            ("window.open", [url, window @ ..]) => url_command(std::slice::from_ref(url), window, "_blank")?,
            // In JavaScript a bare stop() is the browser window's.
            _ if bare && self.dialect == Dialect::JavaScript => return None,
            ("stop", []) => Command::Stop,
            ("play", []) => Command::Play,
            ("nextFrame", []) => Command::NextFrame,
            ("prevFrame", []) => Command::PrevFrame,
            ("nextScene", []) => Command::NextScene,
            ("prevScene", []) => Command::PrevScene,
            ("gotoAndPlay" | "gotoAndStop", [frame, rest @ ..]) => {
                let play = name == "gotoAndPlay";
                let (frame, scene) = match (frame, rest) {
                    (frame, []) => (frame, None),
                    // ActionScript 2.0 names the scene first.
                    (Expr::Text(scene), [frame @ Expr::Number(_)]) if self.dialect == Dialect::ActionScript => (frame, Some(scene)),
                    (frame, [Expr::Text(scene)]) => (frame, Some(scene)),
                    _ => return None,
                };
                Command::Goto { frame: self.frame(frame)?, scene: scene.cloned(), play }
            }
            _ => return None,
        };
        // trace and the URL calls are global functions.
        if matches!(command, Command::Trace(_) | Command::OpenUrl { .. }) && !path.is_empty() {
            return None;
        }
        Some(Stmt::Run { path, command })
    }

    fn frame(&self, e: &Expr) -> Option<FrameRef> {
        match e {
            Expr::Text(label) => Some(FrameRef::Label(label.clone())),
            Expr::Number(n) => {
                let first = self.dialect.first() as f64;
                (n.fract() == 0.0 && *n >= first && n - first <= u32::MAX as f64).then(|| FrameRef::Number((n - first) as u32))
            }
            _ => None,
        }
    }

    /// The timeline an expression names, as steps from the script's own.
    fn path(&self, e: &Expr) -> Option<Vec<Step>> {
        let step = |name: &str| match name {
            "parent" | "_parent" => Step::Parent,
            "root" | "_root" | "exportRoot" => Step::Root,
            name => Step::Child(name.to_string()),
        };
        Some(match e {
            Expr::This => Vec::new(),
            Expr::Name(name) if self.aliases.contains(name) => Vec::new(),
            Expr::Name(name) => vec![step(name)],
            Expr::Member(obj, name) => {
                let mut path = self.path(obj)?;
                match step(name) {
                    Step::Root => path = vec![Step::Root],
                    step => path.push(step),
                }
                path
            }
            // ActionScript 3.0's cast, MovieClip(this.parent), leaves the timeline as it is.
            Expr::Call(f, args) if **f == Expr::Name("MovieClip") && args.len() == 1 => self.path(&args[0])?,
            Expr::Call(f, args) => match (&**f, &args[..]) {
                (Expr::Member(obj, "getChildByName"), [Expr::Text(name)]) => {
                    let mut path = self.path(obj)?;
                    path.push(Step::Child(name.clone()));
                    path
                }
                _ => return None,
            },
            _ => return None,
        })
    }

    fn value(&self, e: &Expr) -> Option<Value> {
        let script = self.dialect == Dialect::ActionScript;
        Some(match e {
            Expr::Text(s) => Value::Text(s.clone()),
            Expr::Number(n) => Value::Number(*n),
            Expr::Add(a, b) => {
                let mut parts = match self.value(a)? {
                    Value::Sum(parts) => parts,
                    v => vec![v],
                };
                parts.push(self.value(b)?);
                Value::Sum(parts)
            }
            Expr::Name("currentFrame" | "_currentframe") if script => Value::CurrentFrame(Vec::new()),
            Expr::Name("totalFrames" | "_totalframes") if script => Value::TotalFrames(Vec::new()),
            Expr::Member(obj, "currentFrame" | "_currentframe") => Value::CurrentFrame(self.path(obj)?),
            Expr::Member(obj, "totalFrames" | "_totalframes") => Value::TotalFrames(self.path(obj)?),
            _ => return None,
        })
    }

    /// The expression at `*i`, before `end`: names, members, calls, `new`, literals and `+`.
    fn expr(&self, i: &mut usize, end: usize) -> Option<Expr<'s>> {
        let mut e = self.unary(i, end)?;
        while *i < end && self.is(*i, "+") {
            *i += 1;
            e = Expr::Add(Box::new(e), Box::new(self.unary(i, end)?));
        }
        Some(e)
    }

    fn unary(&self, i: &mut usize, end: usize) -> Option<Expr<'s>> {
        if *i < end && self.is(*i, "-") {
            *i += 1;
            let Expr::Number(n) = self.unary(i, end)? else { return None };
            return Some(Expr::Number(-n));
        }
        let mut e = self.primary(i, end)?;
        loop {
            if *i + 1 < end && self.is(*i, ".") && self.tok(*i + 1).kind == Lex::Word {
                e = Expr::Member(Box::new(e), self.tok(*i + 1).text);
                *i += 2;
            } else if *i < end && self.is(*i, "(") {
                e = Expr::Call(Box::new(e), self.args(i, end)?);
            } else {
                return Some(e);
            }
        }
    }

    fn primary(&self, i: &mut usize, end: usize) -> Option<Expr<'s>> {
        if *i >= end {
            return None;
        }
        let t = self.tok(*i);
        *i += 1;
        Some(match (t.kind, t.text) {
            (Lex::Word, "this") => Expr::This,
            (Lex::Word, "new") => {
                let class = self.tok(*i);
                if *i >= end || class.kind != Lex::Word {
                    return None;
                }
                *i += 1;
                Expr::New(class.text, if *i < end && self.is(*i, "(") { self.args(i, end)? } else { Vec::new() })
            }
            (Lex::Word, name) => Expr::Name(name),
            (Lex::Text, text) => Expr::Text(unquote(text)?),
            (Lex::Number, text) => Expr::Number(number(text)?),
            (_, "(") => {
                let e = self.expr(i, end)?;
                if !(*i < end && self.is(*i, ")")) {
                    return None;
                }
                *i += 1;
                e
            }
            _ => return None,
        })
    }

    /// A call's arguments, from its `(` to past its `)`.
    fn args(&self, i: &mut usize, end: usize) -> Option<Vec<Expr<'s>>> {
        *i += 1;
        let mut args = Vec::new();
        if *i < end && self.is(*i, ")") {
            *i += 1;
            return Some(args);
        }
        loop {
            args.push(self.expr(i, end)?);
            if *i < end && self.is(*i, ",") {
                *i += 1;
            } else if *i < end && self.is(*i, ")") {
                *i += 1;
                return Some(args);
            } else {
                return None;
            }
        }
    }
}

/// navigateToURL / getURL / window.open with a literal page, and maybe a window.
fn url_command(url: &[Expr], window: &[Expr], default: &str) -> Option<Command> {
    let [Expr::Text(url)] = url else { return None };
    let window = match window {
        [] => default.to_string(),
        [Expr::Text(window)] => window.clone(),
        _ => return None,
    };
    Some(Command::OpenUrl { url: url.clone(), window })
}

/// A string literal's value; None if it isn't closed.
fn unquote(lit: &str) -> Option<String> {
    let quote = lit.chars().next()?;
    let body = lit.strip_prefix(quote)?.strip_suffix(quote)?;
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let code = |chars: &mut std::str::Chars, n: usize| char::from_u32(u32::from_str_radix(&chars.by_ref().take(n).collect::<String>(), 16).ok()?);
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            '0' => out.push('\0'),
            'x' => out.push(code(&mut chars, 2)?),
            'u' => out.push(code(&mut chars, 4)?),
            '\n' => {}
            other => out.push(other),
        }
    }
    Some(out)
}

fn number(text: &str) -> Option<f64> {
    let v = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok()? as f64,
        None => text.parse::<f64>().ok()?,
    };
    v.is_finite().then_some(v)
}

// ---- Timelines as scripts number them -----------------------------------------------

/// A timeline as scripts count its frames: a symbol's, or the movie's, its scenes back to back.
pub struct Track<'a> {
    pub scenes: Vec<Span<'a>>,
}

#[derive(Clone, Copy)]
pub struct Span<'a> {
    /// The scene's name; empty for a symbol's timeline.
    pub name: &'a str,
    pub start: u32,
    pub len: u32,
    pub timeline: &'a Timeline,
}

impl<'a> Track<'a> {
    /// The movie's main timeline: its scenes, as the SWF export plays them one after another.
    pub fn movie(doc: &'a Document) -> Self {
        let mut start = 0;
        let mut scenes = Vec::new();
        for tl in &doc.scenes {
            let len = tl.frame_count();
            scenes.push(Span { name: &tl.name, start, len, timeline: tl });
            start += len;
        }
        Track { scenes }
    }

    pub fn symbol(timeline: &'a Timeline) -> Self {
        Track { scenes: vec![Span { name: "", start: 0, len: timeline.frame_count().max(1), timeline }] }
    }

    pub fn len(&self) -> u32 {
        self.scenes.iter().map(|s| s.len).sum()
    }

    /// The scene holding `frame` (the last, past the end), and the frame within it.
    pub fn at(&self, frame: u32) -> (usize, u32) {
        let s = self.scenes.iter().position(|s| frame < s.start + s.len).unwrap_or(self.scenes.len() - 1);
        (s, frame.saturating_sub(self.scenes[s].start))
    }

    /// Where `command` takes a playhead at `frame`: the frame, and whether it plays on. None
    /// for commands that don't move it, and for a goto to a scene or label there isn't.
    pub fn go(&self, frame: u32, command: &Command) -> Option<(u32, bool)> {
        let (here, _) = self.at(frame);
        Some(match command {
            Command::NextFrame => ((frame + 1).min(self.len().saturating_sub(1)), false),
            Command::PrevFrame => (frame.saturating_sub(1), false),
            Command::NextScene => (self.scenes.get(here + 1)?.start, true),
            Command::PrevScene => (self.scenes[here.checked_sub(1)?].start, true),
            Command::Goto { frame: to, scene, play } => {
                let s = match scene {
                    Some(name) => self.scenes.iter().position(|s| s.name == name)?,
                    None => here,
                };
                let at = match to {
                    FrameRef::Number(n) => self.scenes[s].start + (*n).min(self.scenes[s].len.saturating_sub(1)),
                    // A label in the scene, else (unless a scene was named) anywhere.
                    FrameRef::Label(name) => self.label(name, Some(s)).or_else(|| if scene.is_none() { self.label(name, None) } else { None })?,
                };
                (at, *play)
            }
            _ => return None,
        })
    }

    /// The first frame labelled `name` (a name or anchor, not a comment): in scene `s`, or any.
    pub fn label(&self, name: &str, s: Option<usize>) -> Option<u32> {
        let mut spans = self.scenes.iter().enumerate().filter(|&(i, _)| s.is_none_or(|s| s == i));
        spans.find_map(|(_, span)| {
            let mut keys = span.timeline.layers.iter().flat_map(|l| &l.frames);
            let key = keys.find(|k| k.label().is_some_and(|(n, kind)| n == name && kind != "comment"))?;
            Some(span.start + key.index)
        })
    }

    /// The scripts that run on `frame`: each layer's keyframe starting there, top layer first,
    /// with its layer's name. Guide layers don't publish, so their scripts don't run.
    pub fn scripts(&self, frame: u32, dialect: Dialect) -> Vec<(&'a str, Script)> {
        let (s, local) = self.at(frame);
        let layers = self.scenes[s].timeline.layers.iter().filter(|l| l.kind != LayerKind::Guide);
        layers
            .filter_map(|l| Some((l.name.as_str(), parse(&l.frames.iter().find(|k| k.index == local)?.script()?, dialect))))
            .collect()
    }

    /// Where on it a statement is, as Output notes say: "Scene 1, Layer_1, frame 5, line 2".
    pub fn place(&self, frame: u32, layer: &str, line: usize) -> String {
        let (s, local) = self.at(frame);
        format!("{}, {layer}, frame {}, line {line}", self.scenes[s].timeline.name, local + 1)
    }
}

// ---- JavaScript ---------------------------------------------------------------------

/// The script as JavaScript for the HTML5 Canvas player (html5.rs), a statement a line, run
/// with `this` the timeline it's on. Frames count from 0 there; what's outside the subset
/// stays as comments.
pub fn to_js(script: &Script) -> String {
    let mut out = String::new();
    for stmt in &script.stmts {
        let line = match stmt {
            Stmt::Skip { source, .. } => format!("// Not supported: {source}"),
            Stmt::Run { path, command } => {
                let target = js_path(path);
                match command {
                    Command::Goto { frame, scene, play } => {
                        let frame = match frame {
                            FrameRef::Number(n) => n.to_string(),
                            FrameRef::Label(label) => js_str(label),
                        };
                        let scene = scene.as_ref().map_or(String::new(), |s| format!(", {}", js_str(s)));
                        format!("{target}.{}({frame}{scene});", if *play { "gotoAndPlay" } else { "gotoAndStop" })
                    }
                    Command::Trace(values) => {
                        let values: Vec<_> = values.iter().map(|v| js_value(v, script.dialect)).collect();
                        format!("console.log({});", values.join(", "))
                    }
                    Command::OpenUrl { url, window } => format!("window.open({}, {});", js_str(url), js_str(window)),
                    command => format!("{target}.{}();", command.name()),
                }
            }
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn js_path(path: &[Step]) -> String {
    let mut out = String::from("this");
    for step in path {
        match step {
            Step::Parent => out.push_str(".parent"),
            Step::Root => out = "exportRoot".into(),
            Step::Child(name) => out.push_str(&format!(".getChildByName({})", js_str(name))),
        }
    }
    out
}

fn js_value(v: &Value, dialect: Dialect) -> String {
    match v {
        Value::Text(s) => js_str(s),
        Value::Number(n) => n.to_string(),
        // The player counts frames from 0; ActionScript, from 1.
        Value::CurrentFrame(path) if dialect == Dialect::ActionScript => format!("({}.currentFrame + 1)", js_path(path)),
        Value::CurrentFrame(path) => format!("{}.currentFrame", js_path(path)),
        Value::TotalFrames(path) => format!("{}.totalFrames", js_path(path)),
        Value::Sum(parts) => format!("({})", parts.iter().map(|p| js_value(p, dialect)).collect::<Vec<_>>().join(" + ")),
    }
}

/// A JavaScript string literal.
pub(crate) fn js_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // `<` keeps `</script>` out; the line separators can't sit in a string literal.
            '<' | '\u{2028}' | '\u{2029}' | '\0'..='\u{1f}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---- AVM1 ---------------------------------------------------------------------------

/// The script as AVM1 actions: a DoAction tag's body, End included. Gotos on its own
/// timeline resolve to frame numbers by `track` from frame `here`, as Animate's compiler
/// resolves them; on another timeline they go by number or label as the player finds it.
pub fn to_avm1(script: &Script, track: &Track, here: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut w = Writer::new(&mut out, 11);
    for stmt in &script.stmts {
        let Stmt::Run { path, command } = stmt else { continue };
        let own = path.is_empty();
        let target = slash_path(path);
        let told = !own && !matches!(command, Command::Trace(_) | Command::OpenUrl { .. });
        if told {
            put(&mut w, Action::SetTarget(SetTarget { target: SwfStr::from_utf8_str(&target) }));
        }
        match command {
            Command::Stop => put(&mut w, Action::Stop),
            Command::Play => put(&mut w, Action::Play),
            Command::NextFrame => put(&mut w, Action::NextFrame),
            Command::PrevFrame => put(&mut w, Action::PreviousFrame),
            Command::Goto { .. } | Command::NextScene | Command::PrevScene if own => {
                if let Some((frame, play)) = track.go(here, command) {
                    goto(&mut w, frame, play);
                }
            }
            Command::Goto { frame: FrameRef::Number(n), scene: None, play } => goto(&mut w, *n, *play),
            Command::Goto { frame: FrameRef::Label(label), scene: None, play } => {
                put(&mut w, Action::GotoLabel(GotoLabel { label: SwfStr::from_utf8_str(label) }));
                put(&mut w, if *play { Action::Play } else { Action::Stop });
            }
            // ponytail: another timeline's scene isn't known until it plays, so the SWF export reports
            // these (it makes a root goto naming a scene a frame first). Actions comparing the
            // target's _currentframe with its scenes' starts would play them.
            Command::Goto { .. } | Command::NextScene | Command::PrevScene => {}
            Command::Trace(values) => {
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        push(&mut w, Pushed::Str(SwfStr::from_utf8_str(" ")));
                        put(&mut w, Action::Add2);
                    }
                    avm1_value(&mut w, v, script.dialect);
                    if i > 0 {
                        put(&mut w, Action::Add2);
                    }
                }
                if values.is_empty() {
                    push(&mut w, Pushed::Str(SwfStr::from_utf8_str("")));
                }
                put(&mut w, Action::Trace);
            }
            Command::OpenUrl { url, window } => {
                put(&mut w, Action::GetUrl(GetUrl { url: SwfStr::from_utf8_str(url), target: SwfStr::from_utf8_str(window) }))
            }
        }
        if told {
            put(&mut w, Action::SetTarget(SetTarget { target: SwfStr::from_utf8_str("") }));
        }
    }
    put(&mut w, Action::End);
    out
}

fn put(w: &mut Writer<&mut Vec<u8>>, action: Action) {
    w.write_action(&action).expect("writing to memory");
}

/// GotoFrame (from 0) then Play or Stop.
fn goto(w: &mut Writer<&mut Vec<u8>>, frame: u32, play: bool) {
    put(w, Action::GotoFrame(GotoFrame { frame: frame.min(u16::MAX as u32) as u16 }));
    put(w, if play { Action::Play } else { Action::Stop });
}

fn push(w: &mut Writer<&mut Vec<u8>>, value: Pushed) {
    put(w, Action::Push(Push { values: vec![value] }));
}

/// Pushes `v`'s value.
fn avm1_value(w: &mut Writer<&mut Vec<u8>>, v: &Value, dialect: Dialect) {
    match v {
        Value::Text(s) => push(w, Pushed::Str(SwfStr::from_utf8_str(s))),
        Value::Number(n) if n.fract() == 0.0 && n.abs() < i32::MAX as f64 => push(w, Pushed::Int(*n as i32)),
        Value::Number(n) => push(w, Pushed::Double(*n)),
        Value::CurrentFrame(path) | Value::TotalFrames(path) => {
            // GetProperty's _currentframe (4) and _totalframes (5); AVM1 counts frames from 1.
            let target = slash_path(path);
            let property = if matches!(v, Value::CurrentFrame(_)) { 4 } else { 5 };
            put(w, Action::Push(Push { values: vec![Pushed::Str(SwfStr::from_utf8_str(&target)), Pushed::Int(property)] }));
            put(w, Action::GetProperty);
            if property == 4 && dialect == Dialect::JavaScript {
                push(w, Pushed::Int(1));
                put(w, Action::Subtract);
            }
        }
        Value::Sum(parts) => {
            for (i, part) in parts.iter().enumerate() {
                avm1_value(w, part, dialect);
                if i > 0 {
                    put(w, Action::Add2);
                }
            }
        }
    }
}

/// `path` as an AVM1 target in slash syntax: "", "..", "/", "ball", "../ball".
fn slash_path(path: &[Step]) -> String {
    let (mut parts, mut rooted) = (Vec::new(), false);
    for step in path {
        match step {
            Step::Root => (parts, rooted) = (Vec::new(), true),
            Step::Parent => parts.push(".."),
            Step::Child(name) => parts.push(name.as_str()),
        }
    }
    let joined = parts.join("/");
    if rooted { format!("/{joined}") } else { joined }
}

/// What the SWF export adds to the main timeline (None) and movie clips' sprites, by id.
pub(crate) struct SwfFrames<'a> {
    timelines: HashMap<Option<CharacterId>, SwfTimeline<'a>>,
}

/// A timeline's labels, as (name, anchor), and compiled scripts by frame, which go in before the
/// frame's ShowFrame; and the instance names, by frame and depth, of the clips it places.
struct SwfTimeline<'a> {
    frames: BTreeMap<u32, (Vec<(&'a str, bool)>, Vec<u8>)>,
    names: Vec<Vec<Option<String>>>,
}

impl<'a> SwfFrames<'a> {
    /// The main timeline's and those of `sprites` (movie clips exported as sprites, by id),
    /// adding the statements skipped to `skipped`.
    pub(crate) fn new(doc: &'a Document, sprites: impl IntoIterator<Item = (CharacterId, &'a Symbol)>, skipped: &mut BTreeSet<String>) -> Self {
        let dialect = Dialect::of(doc);
        let movie = Track::movie(doc);
        let mut timelines = HashMap::new();
        let mut add = |id, track: &Track<'a>| {
            let timeline = SwfTimeline { frames: swf_frames(track, &movie, dialect, skipped), names: swf_names(doc, track) };
            timelines.insert(id, timeline);
        };
        add(None, &movie);
        for (id, symbol) in sprites {
            add(Some(id), &Track::symbol(&symbol.timeline));
        }
        SwfFrames { timelines }
    }

    /// `tags`, the main timeline's (`sprite` None) or a sprite's, with each frame's labels and
    /// DoAction in before its ShowFrame, and clips placed under their instance names; the same
    /// goes into every sprite defined among them.
    pub(crate) fn splice<'b>(&'b self, tags: Vec<Tag<'b>>, sprite: Option<CharacterId>) -> Vec<Tag<'b>> {
        let timeline = self.timelines.get(&sprite);
        let mut out = Vec::with_capacity(tags.len());
        let mut frame = 0;
        for tag in tags {
            match tag {
                Tag::DefineSprite(mut s) => {
                    s.tags = self.splice(std::mem::take(&mut s.tags), Some(s.id));
                    out.push(Tag::DefineSprite(s));
                }
                // Scripts reach a clip by its instance name.
                Tag::PlaceObject(mut place) if matches!(place.action, PlaceObjectAction::Place(_) | PlaceObjectAction::Replace(_)) => {
                    let names = timeline.and_then(|t| t.names.get(frame as usize));
                    if let Some(Some(name)) = names.and_then(|n| n.get(usize::from(place.depth).wrapping_sub(1))) {
                        place.name = Some(SwfStr::from_utf8_str(name));
                    }
                    out.push(Tag::PlaceObject(place));
                }
                Tag::ShowFrame => {
                    if let Some((labels, actions)) = timeline.and_then(|t| t.frames.get(&frame)) {
                        let labels = labels.iter().map(|&(label, is_anchor)| FrameLabel { label: SwfStr::from_utf8_str(label), is_anchor });
                        out.extend(labels.map(Tag::FrameLabel));
                        if !actions.is_empty() {
                            out.push(Tag::DoAction(actions.as_slice()));
                        }
                    }
                    frame += 1;
                    out.push(Tag::ShowFrame);
                }
                tag => out.push(tag),
            }
        }
        out
    }
}

/// A timeline's labels and compiled scripts, by frame.
fn swf_frames<'a>(track: &Track<'a>, movie: &Track, dialect: Dialect, skipped: &mut BTreeSet<String>) -> BTreeMap<u32, (Vec<(&'a str, bool)>, Vec<u8>)> {
    let main = std::ptr::eq(track, movie);
    let mut out: BTreeMap<u32, (Vec<(&'a str, bool)>, Vec<u8>)> = BTreeMap::new();
    let mut scripted = BTreeSet::new();
    for span in &track.scenes {
        for key in span.timeline.layers.iter().flat_map(|l| &l.frames) {
            let at = span.start + key.index;
            if let Some((name, kind)) = key.label().filter(|&(_, kind)| kind != "comment") {
                out.entry(at).or_default().0.push((name, kind == "anchor"));
            }
            if key.script().is_some() {
                scripted.insert(at);
            }
        }
    }
    for at in scripted {
        let mut all = Script { dialect, stmts: Vec::new() };
        for (layer, script) in track.scripts(at, dialect) {
            for stmt in script.stmts {
                match stmt {
                    Stmt::Skip { line, source } => {
                        skipped.insert(format!("script `{source}` ({})", track.place(at, layer, line)));
                    }
                    Stmt::Run { mut path, mut command } => {
                        // The main timeline's scenes are known here: on it the root is the script's
                        // own timeline, and a root goto naming a scene is a frame of it.
                        if main && path.first() == Some(&Step::Root) {
                            path.remove(0);
                        } else if path == [Step::Root]
                            && let Command::Goto { scene: Some(_), play, .. } = command
                            && let Some((frame, _)) = movie.go(0, &command)
                        {
                            command = Command::Goto { frame: FrameRef::Number(frame), scene: None, play };
                        }
                        // Another timeline's scene is only known where its playhead is.
                        if !path.is_empty() && matches!(command, Command::NextScene | Command::PrevScene | Command::Goto { scene: Some(_), .. }) {
                            let (s, local) = track.at(at);
                            let place = format!("{}, {layer}, frame {}", track.scenes[s].timeline.name, local + 1);
                            skipped.insert(format!("{}() of another timeline's scenes ({place})", command.name()));
                            continue;
                        }
                        all.stmts.push(Stmt::Run { path, command });
                    }
                }
            }
        }
        if !all.stmts.is_empty() {
            out.entry(at).or_default().1 = to_avm1(&all, track, at);
        }
    }
    out
}

/// Each frame's instance names by depth (from 1), for the clips placed under one; none
/// when the timeline names no instance.
fn swf_names(doc: &Document, track: &Track) -> Vec<Vec<Option<String>>> {
    let keys = track.scenes.iter().flat_map(|s| &s.timeline.layers).flat_map(|l| &l.frames);
    if !keys.flat_map(|k| &k.elements).any(|e| matches!(e, Element::Instance(i) if i.extra.get("name").is_some())) {
        return Vec::new();
    }
    let names = |f| {
        let (s, local) = track.at(f);
        let tl = track.scenes[s].timeline;
        let list = display_list(doc, tl, local, &mut BTreeSet::new());
        let mut placed = Vec::new();
        depths(&list, &mut placed);
        placed.iter().map(|p| if let Item::Clip(..) = p.item { crate::player::instance_name(tl, p.source) } else { None }).collect()
    };
    (0..track.len()).map(names).collect()
}

/// `list`'s items in the order swf_export gives them depths: a mask at work takes one, then
/// what it masks, and an empty mask hides what it masks.
// ponytail: this mirrors swf_export's `slots`, so the two change together (scripting.rs names
// a clip under a mask); the export's own slots carrying names would end the mirror.
fn depths<'p, 'a>(list: &'p [Placed<'a>], out: &mut Vec<&'p Placed<'a>>) {
    for p in list {
        match &p.item {
            Item::Masked { mask, content } => {
                let mut held = Vec::new();
                depths(mask, &mut held);
                if !held.is_empty() {
                    out.push(p);
                    depths(content, out);
                }
            }
            _ => out.push(p),
        }
    }
}

#[cfg(test)]
mod tests {
    use swf::avm1::read::Reader;

    use super::*;

    fn actions(bytes: &[u8]) -> Vec<String> {
        let mut r = Reader::new(bytes, 11);
        let mut out = Vec::new();
        loop {
            let action = r.read_action().unwrap();
            if action == Action::End {
                return out;
            }
            out.push(format!("{action:?}"));
        }
    }

    fn runs(src: &str, dialect: Dialect) -> Vec<Stmt> {
        parse(src, dialect).stmts
    }

    fn run(path: Vec<Step>, command: Command) -> Stmt {
        Stmt::Run { path, command }
    }

    #[test]
    fn the_subset_reads_in_both_dialects() {
        use Command::*;
        let goto = |frame, scene: Option<&str>, play| Goto { frame, scene: scene.map(String::from), play };
        let src = r#"
            stop(); play()
            gotoAndPlay(5); gotoAndStop("intro", "Scene 2")
            this.nextFrame(); MovieClip(this.parent).prevScene();
            ball.gotoAndStop(1); root.nextScene(); _parent._parent.play();
            trace("frame", currentFrame + 1, 'it''s');
            navigateToURL(new URLRequest("https://example.com"), "_self"); getURL("http://a.b");
            gotoAndPlay("Scene 3", 2);
        "#;
        let parent = || vec![Step::Parent];
        assert_eq!(runs(src, Dialect::ActionScript), [
            run(vec![], Stop),
            run(vec![], Play),
            run(vec![], goto(FrameRef::Number(4), None, true)),
            run(vec![], goto(FrameRef::Label("intro".into()), Some("Scene 2"), false)),
            run(vec![], NextFrame),
            run(parent(), PrevScene),
            run(vec![Step::Child("ball".into())], goto(FrameRef::Number(0), None, false)),
            run(vec![Step::Root], NextScene),
            run(vec![Step::Parent, Step::Parent], Play),
            Stmt::Skip { line: 6, source: "trace(\"frame\", currentFrame + 1, 'it''s');".into() },
            run(vec![], OpenUrl { url: "https://example.com".into(), window: "_self".into() }),
            run(vec![], OpenUrl { url: "http://a.b".into(), window: "_self".into() }),
            run(vec![], goto(FrameRef::Number(1), Some("Scene 3"), true)),
        ]);
        let traced = runs(r#"trace("frame", this.currentFrame + 1, "\x41B")"#, Dialect::ActionScript);
        let values = vec![Value::Text("frame".into()), Value::Sum(vec![Value::CurrentFrame(vec![]), Value::Number(1.0)]), Value::Text("AB".into())];
        assert_eq!(traced, [run(vec![], Trace(values))]);

        // JavaScript counts frames from 0 and needs `this.`: a bare stop() would be the window's.
        let js = "this.gotoAndPlay(0); stop(); exportRoot.gotoAndStop(3);\nconsole.log(this.currentFrame); window.open('x.html');";
        assert_eq!(runs(js, Dialect::JavaScript), [
            run(vec![], goto(FrameRef::Number(0), None, true)),
            Stmt::Skip { line: 1, source: "stop();".into() },
            run(vec![Step::Root], goto(FrameRef::Number(3), None, false)),
            run(vec![], Trace(vec![Value::CurrentFrame(vec![])])),
            run(vec![], OpenUrl { url: "x.html".into(), window: "_blank".into() }),
        ]);
        // Frame 0 doesn't exist in ActionScript.
        assert!(matches!(runs("gotoAndStop(0);", Dialect::ActionScript)[..], [Stmt::Skip { .. }]));
    }

    #[test]
    fn what_the_subset_doesnt_run_is_skipped_whole() {
        let src = "import flash.events.MouseEvent;\n\
                   var n:int = 3; // a comment, stop();\n\
                   if (n > 2)\n    stop();\nelse { play(); }\n\
                   button.addEventListener(MouseEvent.CLICK, function (e) {\n  gotoAndPlay(2);\n});\n\
                   function go(e:MouseEvent):void\n{\n  nextFrame();\n}\n\
                   i++
/* stop(); */ play()";
        let stmts = runs(src, Dialect::ActionScript);
        let skipped: Vec<_> = stmts.iter().filter_map(|s| if let Stmt::Skip { line, source } = s { Some((*line, source.as_str())) } else { None }).collect();
        assert_eq!(skipped, [
            (2, "var n:int = 3;"),
            (3, "if (n > 2) stop(); else { play(); }"),
            (6, "button.addEventListener(MouseEvent.CLICK, function (e) { gotoAndPlay(2); });"),
            (9, "function go(e:MouseEvent):void { nextFrame(); }"),
            (13, "i++"),
        ]);
        assert_eq!(stmts.last(), Some(&run(vec![], Command::Play)), "only the last line runs");
        assert_eq!(stmts.len(), 6, "`i++` ends its line");
    }

    #[test]
    fn a_name_for_this_is_the_scripts_own_timeline() {
        // Animate's HTML5 Canvas snippets start with `var _this = this;`; ActionScript can type it.
        let js = "var _this = this;\n_this.stop();\n_this.ball.gotoAndPlay(0);";
        let goto = Command::Goto { frame: FrameRef::Number(0), scene: None, play: true };
        assert_eq!(runs(js, Dialect::JavaScript), [run(vec![], Command::Stop), run(vec![Step::Child("ball".into())], goto)]);
        assert_eq!(to_js(&parse(js, Dialect::JavaScript)), "this.stop();\nthis.getChildByName(\"ball\").gotoAndPlay(0);\n");
        assert_eq!(runs("var self:MovieClip = this; self.play();", Dialect::ActionScript), [run(vec![], Command::Play)]);
    }

    #[test]
    fn nesting_too_deep_for_the_stack_is_skipped() {
        let n = 20_000;
        for src in [
            format!("trace({}1{});", "(".repeat(n), ")".repeat(n)),
            format!("{}stop();{}", "{".repeat(n), "}".repeat(n)),
            format!("{}stop();", "if (a) ".repeat(n)),
            format!("trace(1{});", "+1".repeat(n)),
        ] {
            let stmts = runs(&src, Dialect::ActionScript);
            assert!(!stmts.is_empty() && stmts.iter().all(|s| matches!(s, Stmt::Skip { .. })), "{src:.40}");
        }
        // Blocks as scripts nest them still run.
        assert_eq!(runs("{ { stop(); } }", Dialect::ActionScript), [run(vec![], Command::Stop)]);
    }

    #[test]
    fn scripts_round_trip_through_the_frame() {
        let mut frame = crate::doc::Timeline::new("t").layers.remove(0).frames.remove(0);
        assert_eq!(frame.script(), None);
        let text = "if (a < b && c > d) stop(); // ]]> \"q\" é\r\n\ttrace('x')\r\r]]]]>\n";
        frame.set_script(text);
        assert_eq!(frame.script().as_deref(), Some(text));
        frame.set_script("stop();");
        assert_eq!((frame.script().as_deref(), frame.extra.before.len()), (Some("stop();"), 1), "replaced in place");
        // A pasted control character XML can't hold goes, rather than the script (and the file) with it.
        frame.set_script("stop();\u{c}\u{1}play();\u{ffff}");
        assert_eq!((frame.script().as_deref(), frame.extra.before.len()), (Some("stop();play();"), 1));
        frame.set_script("");
        assert_eq!((frame.script(), frame.extra.before.len()), (None, 0));
    }

    #[test]
    fn avm1_actions() {
        let doc = Document::new(550.0, 400.0, 24.0);
        let mut tl = Timeline::new("Clip");
        let layer = &mut tl.layers[0];
        layer.insert_frames(0, 9);
        layer.insert_keyframe(6);
        layer.frames[1].set_label("end", "name");
        let track = Track::symbol(&tl);
        let compile = |src: &str| actions(&to_avm1(&parse(src, Dialect::ActionScript), &track, 0));
        assert_eq!(compile("stop(); play();"), ["Stop", "Play"]);
        assert_eq!(compile("gotoAndPlay(3); gotoAndStop(\"end\");"), [
            "GotoFrame(GotoFrame { frame: 2 })",
            "Play",
            "GotoFrame(GotoFrame { frame: 6 })",
            "Stop"
        ]);
        // Another timeline: told, by number or label.
        assert_eq!(compile("MovieClip(this.parent).ball.gotoAndStop(\"x\");"), [
            "SetTarget(SetTarget { target: \"../ball\" })",
            "GotoLabel(GotoLabel { label: \"x\" })",
            "Stop",
            "SetTarget(SetTarget { target: \"\" })"
        ]);
        assert_eq!(compile("trace(\"at \" + currentFrame, 2);"), [
            "Push(Push { values: [Str(\"at \")] })",
            "Push(Push { values: [Str(\"\"), Int(4)] })",
            "GetProperty",
            "Add2",
            "Push(Push { values: [Str(\" \")] })",
            "Add2",
            "Push(Push { values: [Int(2)] })",
            "Add2",
            "Trace"
        ]);
        assert_eq!(compile("navigateToURL(new URLRequest(\"https://a.b\"));"), ["GetUrl(GetUrl { url: \"https://a.b\", target: \"_blank\" })"]);
        // Scenes resolve on the main timeline.
        let mut doc = doc;
        doc.scenes[0].layers[0].insert_frames(0, 4);
        doc.insert_scene(0);
        let movie = Track::movie(&doc);
        let src = parse("nextScene(); gotoAndStop(2, \"Scene 2\"); prevScene(); gotoAndPlay(1, \"Nope\");", Dialect::ActionScript);
        assert_eq!(actions(&to_avm1(&src, &movie, 0)), ["GotoFrame(GotoFrame { frame: 5 })", "Play", "GotoFrame(GotoFrame { frame: 5 })", "Stop"]);
    }

    #[test]
    fn javascript() {
        let src = "stop();\ngotoAndPlay(\"intro\", \"Scene 2\");\nMovieClip(parent).ball.nextFrame();\n\
                   trace(\"f\" + currentFrame, root.totalFrames);\nnavigateToURL(new URLRequest(\"a</script>\"));\nx++;";
        assert_eq!(to_js(&parse(src, Dialect::ActionScript)), "this.stop();\n\
             this.gotoAndPlay(\"intro\", \"Scene 2\");\n\
             this.parent.getChildByName(\"ball\").nextFrame();\n\
             console.log((\"f\" + (this.currentFrame + 1)), exportRoot.totalFrames);\n\
             window.open(\"a\\u003c/script>\", \"_blank\");\n\
             // Not supported: x++;\n");
    }

    #[test]
    fn colouring_tokens() {
        let src = "stop(); // done\n\"a\\\"b\" 0x1F 1e-3 /* c */ 'open";
        let kinds: Vec<_> = lex(src).into_iter().map(|(k, r)| (k, &src[r])).collect();
        assert_eq!(kinds, [
            (Lex::Word, "stop"),
            (Lex::Punct, "("),
            (Lex::Punct, ")"),
            (Lex::Punct, ";"),
            (Lex::Comment, "// done"),
            (Lex::Text, "\"a\\\"b\""),
            (Lex::Number, "0x1F"),
            (Lex::Number, "1e-3"),
            (Lex::Comment, "/* c */"),
            (Lex::Text, "'open"),
        ]);
    }
}
