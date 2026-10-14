//! Curve expressions for the Motion Editor, after OpenToonz's expression
//! grammar (`common/expressions/tgrammar.cpp`; BSD-3-Clause, © DWANGO and
//! OpenToonz contributors; see THIRD-PARTY-NOTICES.md).
//!
//! Same operators, precedences and functions: `+ - * / ^ %`, comparisons,
//! `&& || and or ! not`, `c ? a : b`; `sin cos tan sinh cosh tanh` (degrees),
//! `atan atan2` (degrees), `log exp floor ceil ceiling round abs sign sqrt sqr
//! min max crop clamp step smoothstep pulse saw sawtooth wave`, the random
//! families `random rnd random_s rnd_s random_p rnd_p random_ps rnd_ps`, and
//! `cycle(period)`. Variables: `t` (0…1 across the segment), `frame`/`f`
//! (frame number, 1-based), `rframe`/`r` (frame within the segment, 1-based),
//! `pi`. AnimationCraft adds `v0`/`v1` (the segment's key values) and the
//! object's other properties by name (`rotation`, `x`, `alpha`…), optionally
//! at another frame: `rotation(frame - 5)`.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
    Question,
    Colon,
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = vec![];
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let st = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            if i < c.len() && (c[i] == 'e' || c[i] == 'E') && c.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == '-' || *d == '+') {
                i += 2;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let txt: String = c[st..i].iter().collect();
            out.push(Tok::Num(txt.parse().map_err(|_| format!("bad number '{txt}'"))?));
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let st = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '.') {
                i += 1;
            }
            out.push(Tok::Id(c[st..i].iter().collect::<String>().to_ascii_lowercase()));
            continue;
        }
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        let op2 = ["<=", ">=", "==", "!=", "&&", "||"].into_iter().find(|o| *o == two);
        if let Some(o) = op2 {
            out.push(Tok::Op(o));
            i += 2;
            continue;
        }
        out.push(match ch {
            '+' => Tok::Op("+"),
            '-' => Tok::Op("-"),
            '*' => Tok::Op("*"),
            '/' => Tok::Op("/"),
            '^' => Tok::Op("^"),
            '%' => Tok::Op("%"),
            '<' => Tok::Op("<"),
            '>' => Tok::Op(">"),
            '!' => Tok::Op("!"),
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            ',' => Tok::Comma,
            '?' => Tok::Question,
            ':' => Tok::Colon,
            _ => return Err(format!("unexpected '{ch}'")),
        });
        i += 1;
    }
    Ok(out)
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Num(f64),
    Var(String),
    Neg(Box<Node>),
    Not(Box<Node>),
    Bin(&'static str, Box<Node>, Box<Node>),
    Cond(Box<Node>, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

/// OpenToonz's operator priorities.
fn prec(op: &str) -> Option<u8> {
    Some(match op {
        "||" | "or" => 2,
        "&&" | "and" => 3,
        "<" | ">" | "<=" | ">=" | "==" | "!=" => 6,
        "%" => 8,
        "+" | "-" => 10,
        "*" | "/" => 20,
        "^" => 30,
        _ => return None,
    })
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.i).cloned();
        self.i += 1;
        t
    }
    fn expect(&mut self, t: Tok, what: &str) -> Result<(), String> {
        match self.next() {
            Some(x) if x == t => Ok(()),
            _ => Err(format!("expected {what}")),
        }
    }
    /// expression with the ternary on top
    fn expr(&mut self) -> Result<Node, String> {
        let c = self.binary(0)?;
        if self.peek() == Some(&Tok::Question) {
            self.next();
            let a = self.expr()?;
            self.expect(Tok::Colon, "':' in 'c ? a : b'")?;
            let b = self.expr()?;
            return Ok(Node::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }
    fn op_here(&self) -> Option<&'static str> {
        match self.peek()? {
            Tok::Op(o) if *o != "!" => Some(o),
            Tok::Id(s) if s == "and" => Some("and"),
            Tok::Id(s) if s == "or" => Some("or"),
            _ => None,
        }
    }
    fn binary(&mut self, min: u8) -> Result<Node, String> {
        let mut lhs = self.unary()?;
        while let Some(op) = self.op_here() {
            let p = prec(op).unwrap_or(0);
            if p < min {
                break;
            }
            self.next();
            // ^ is right-associative
            let rhs = self.binary(if op == "^" { p } else { p + 1 })?;
            let op = match op {
                "and" => "&&",
                "or" => "||",
                o => o,
            };
            lhs = Node::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
    fn unary(&mut self) -> Result<Node, String> {
        match self.peek() {
            Some(Tok::Op("-")) => {
                self.next();
                Ok(Node::Neg(Box::new(self.binary(25)?)))
            }
            Some(Tok::Op("+")) => {
                self.next();
                self.binary(25)
            }
            Some(Tok::Op("!")) => {
                self.next();
                Ok(Node::Not(Box::new(self.binary(25)?)))
            }
            Some(Tok::Id(s)) if s == "not" => {
                self.next();
                Ok(Node::Not(Box::new(self.binary(25)?)))
            }
            _ => self.atom(),
        }
    }
    fn atom(&mut self) -> Result<Node, String> {
        match self.next() {
            Some(Tok::Num(v)) => Ok(Node::Num(v)),
            Some(Tok::LParen) => {
                let e = self.expr()?;
                self.expect(Tok::RParen, "')'")?;
                Ok(e)
            }
            Some(Tok::Id(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.next();
                    let mut args = vec![];
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.expr()?);
                            match self.next() {
                                Some(Tok::Comma) => continue,
                                Some(Tok::RParen) => break,
                                _ => return Err(format!("expected ',' or ')' in {name}(…)")),
                            }
                        }
                    } else {
                        self.next();
                    }
                    Ok(Node::Call(name, args))
                } else {
                    Ok(Node::Var(name))
                }
            }
            Some(t) => Err(format!("unexpected {}", describe(&t))),
            None => Err("unexpected end of expression".into()),
        }
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Num(v) => format!("number {v}"),
        Tok::Id(s) => format!("'{s}'"),
        Tok::Op(o) => format!("'{o}'"),
        Tok::LParen => "'('".into(),
        Tok::RParen => "')'".into(),
        Tok::Comma => "','".into(),
        Tok::Question => "'?'".into(),
        Tok::Colon => "':'".into(),
    }
}

/// Where an expression is evaluated.
pub struct Ctx<'a> {
    /// Frame number (1-based).
    pub frame: f64,
    /// Frame within the segment (1-based).
    pub rframe: f64,
    /// 0…1 across the segment.
    pub t: f64,
    /// The segment's key values.
    pub v0: f64,
    pub v1: f64,
    /// Another property of the object at a frame (1-based), by name.
    pub refs: &'a dyn Fn(&str, f64) -> Option<f64>,
    /// This curve at a frame (1-based): `cycle()`.
    pub own: &'a dyn Fn(f64) -> Option<f64>,
}

const FUNCS: &[&str] = &[
    "sin", "cos", "tan", "sinh", "cosh", "tanh", "atan", "atan2", "log", "exp", "floor", "ceil", "ceiling", "round", "abs", "sign", "sqrt", "sqr", "crop", "clamp", "min", "max", "step", "smoothstep", "pulse", "saw", "sawtooth", "wave", "random", "rnd", "random_s", "rnd_s", "random_p", "rnd_p", "random_ps", "rnd_ps", "cycle",
];

/// A parsed expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    root: Node,
    src: String,
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.src)
    }
}

/// OpenToonz's random sequence: a value in [0, 1) for (seed, integer step).
fn rand01(seed: f64, i: f64) -> f64 {
    let mut h = (seed.to_bits() ^ 0x9E37_79B9_7F4A_7C15).wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ ((i.max(0.0).floor() as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    h ^= h >> 31;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

impl Expr {
    /// Parse; `props` are the property names the expression may reference.
    pub fn parse(src: &str, props: &[&str]) -> Result<Expr, String> {
        let toks = tokenize(src)?;
        if toks.is_empty() {
            return Err("empty expression".into());
        }
        let mut p = Parser { toks, i: 0 };
        let root = p.expr()?;
        if let Some(t) = p.peek() {
            return Err(format!("unexpected {} after the expression", describe(t)));
        }
        check(&root, props)?;
        Ok(Expr { root, src: src.into() })
    }

    pub fn eval(&self, c: &Ctx) -> f64 {
        let v = ev(&self.root, c, 0);
        if v.is_finite() {
            v
        } else {
            0.0
        }
    }
}

fn check(n: &Node, props: &[&str]) -> Result<(), String> {
    match n {
        Node::Num(_) => Ok(()),
        Node::Var(v) => {
            if ["t", "f", "frame", "r", "rframe", "pi", "e", "v0", "v1"].contains(&v.as_str()) || props.contains(&v.as_str()) {
                Ok(())
            } else {
                Err(format!("unknown name '{v}'"))
            }
        }
        Node::Neg(a) | Node::Not(a) => check(a, props),
        Node::Bin(_, a, b) => {
            check(a, props)?;
            check(b, props)
        }
        Node::Cond(a, b, c) => {
            check(a, props)?;
            check(b, props)?;
            check(c, props)
        }
        Node::Call(name, args) => {
            if props.contains(&name.as_str()) {
                if args.len() != 1 {
                    return Err(format!("{name}(frame) takes one argument"));
                }
            } else if !FUNCS.contains(&name.as_str()) {
                return Err(format!("unknown function '{name}'"));
            } else {
                let n = args.len();
                let ok = match name.as_str() {
                    "atan2" | "min" | "max" | "step" | "wave" => n == 2,
                    "crop" | "clamp" | "smoothstep" | "pulse" | "saw" | "sawtooth" => n == 3,
                    "random" | "rnd" => n <= 2,
                    "random_s" | "rnd_s" | "random_p" | "rnd_p" => (1..=3).contains(&n),
                    "random_ps" | "rnd_ps" => (2..=4).contains(&n),
                    _ => n == 1,
                };
                if !ok {
                    return Err(format!("wrong number of arguments for {name}()"));
                }
            }
            args.iter().try_for_each(|a| check(a, props))
        }
    }
}

fn ev(n: &Node, c: &Ctx, depth: u32) -> f64 {
    if depth > 200 {
        return 0.0;
    }
    let e = |n: &Node| ev(n, c, depth + 1);
    let b = |x: bool| if x { 1.0 } else { 0.0 };
    match n {
        Node::Num(v) => *v,
        Node::Var(v) => match v.as_str() {
            "t" => c.t,
            "f" | "frame" => c.frame,
            "r" | "rframe" => c.rframe,
            "pi" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            "v0" => c.v0,
            "v1" => c.v1,
            name => (c.refs)(name, c.frame).unwrap_or(0.0),
        },
        Node::Neg(a) => -e(a),
        Node::Not(a) => b(e(a) == 0.0),
        Node::Bin(op, x, y) => {
            let (x, y) = (e(x), e(y));
            match *op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => {
                    if y == 0.0 {
                        0.0
                    } else {
                        x / y
                    }
                }
                "^" => x.powf(y),
                "%" => {
                    if y == 0.0 {
                        0.0
                    } else {
                        x - y * (x / y).floor()
                    }
                }
                "<" => b(x < y),
                ">" => b(x > y),
                "<=" => b(x <= y),
                ">=" => b(x >= y),
                "==" => b(x == y),
                "!=" => b(x != y),
                "&&" => b(x != 0.0 && y != 0.0),
                "||" => b(x != 0.0 || y != 0.0),
                _ => 0.0,
            }
        }
        Node::Cond(k, a, z) => {
            if e(k) != 0.0 {
                e(a)
            } else {
                e(z)
            }
        }
        Node::Call(name, args) => {
            let a: Vec<f64> = args.iter().map(e).collect();
            let deg = |x: f64| x.to_radians();
            let random = |seed: f64, arg: f64, rest: &[f64]| -> f64 {
                let r = rand01(seed, arg.abs());
                match rest {
                    [] => r,
                    [max] => max * r,
                    [min, max, ..] => (1.0 - r) * min + r * max,
                }
            };
            let periodic = |period: f64, seed: f64, rest: &[f64]| -> f64 {
                let period = period.max(1e-6);
                let f = c.frame / period;
                let (f0, k) = (f.floor(), f - f.floor());
                let r = (1.0 - k) * rand01(seed, f0) + k * rand01(seed, f0 + 1.0);
                match rest {
                    [] => r,
                    [max] => max * r,
                    [min, max, ..] => (1.0 - r) * min + r * max,
                }
            };
            match name.as_str() {
                "sin" => deg(a[0]).sin(),
                "cos" => deg(a[0]).cos(),
                "tan" => deg(a[0]).tan(),
                "sinh" => deg(a[0]).sinh(),
                "cosh" => deg(a[0]).cosh(),
                "tanh" => deg(a[0]).tanh(),
                "atan" => a[0].atan().to_degrees(),
                "atan2" => a[0].atan2(a[1]).to_degrees(),
                "log" => a[0].ln(),
                "exp" => a[0].exp(),
                "floor" => a[0].floor(),
                "ceil" | "ceiling" => a[0].ceil(),
                "round" => a[0].round(),
                "abs" => a[0].abs(),
                "sign" => {
                    if a[0] > 0.0 {
                        1.0
                    } else if a[0] < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                }
                "sqrt" => a[0].max(0.0).sqrt(),
                "sqr" => a[0] * a[0],
                "crop" | "clamp" => a[0].max(a[1]).min(a[2]),
                "min" => a[0].min(a[1]),
                "max" => a[0].max(a[1]),
                "step" => b(a[0] >= a[1]),
                "smoothstep" => {
                    let (v, lo, hi) = (a[0], a[1], a[2]);
                    if v <= lo {
                        0.0
                    } else if v >= hi {
                        1.0
                    } else {
                        let t = (v - lo) / (hi - lo);
                        -2.0 * t * t * t + 3.0 * t * t
                    }
                }
                "pulse" => {
                    let (x, x0, len) = (a[0], a[1], a[2]);
                    let k = (std::f64::consts::LN_2 * 4.0) / (len * len).max(1e-9);
                    (-(x - x0) * (x - x0) * k).exp()
                }
                "saw" | "sawtooth" => {
                    let (x, len, h) = (a[0], a[1], a[2]);
                    if len <= 0.0 {
                        0.0
                    } else {
                        let q = x / len;
                        h * (q - q.floor())
                    }
                }
                "wave" => {
                    if a[1] <= 0.0 {
                        0.0
                    } else {
                        (a[0] * std::f64::consts::TAU / a[1]).sin()
                    }
                }
                "random" | "rnd" => random(0.0, c.frame, &a),
                "random_s" | "rnd_s" => random(a[0], c.frame, &a[1..]),
                "random_p" | "rnd_p" => periodic(a[0], 0.0, &a[1..]),
                "random_ps" | "rnd_ps" => periodic(a[0], a[1], &a[2..]),
                "cycle" => {
                    let d = a[0].max(1.0);
                    (c.own)(c.frame - d).unwrap_or(0.0)
                }
                prop => (c.refs)(prop, a[0]).unwrap_or(0.0),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str, frame: f64) -> f64 {
        let refs = |name: &str, f: f64| if name == "rotation" { Some(f * 10.0) } else { None };
        let own = |f: f64| Some(f);
        let e = Expr::parse(src, &["rotation"]).unwrap_or_else(|e| panic!("{src}: {e}"));
        e.eval(&Ctx { frame, rframe: frame, t: 0.5, v0: 10.0, v1: 20.0, refs: &refs, own: &own })
    }

    #[test]
    fn grammar_matches_opentoonz() {
        assert_eq!(run("1 + 2 * 3", 1.0), 7.0);
        assert_eq!(run("2 ^ 3 ^ 2", 1.0), 512.0, "right-associative power");
        assert_eq!(run("-2 ^ 2", 1.0), -4.0);
        assert_eq!(run("7 % 3 + 1", 1.0), 7.0 % 4.0, "% binds looser than + (OpenToonz)");
        assert_eq!(run("frame > 3 ? 1 : 2", 5.0), 1.0);
        assert_eq!(run("frame > 3 and not (frame > 9)", 5.0), 1.0);
        assert!((run("sin(90)", 1.0) - 1.0).abs() < 1e-12, "degrees");
        assert!((run("atan2(1, 1)", 1.0) - 45.0).abs() < 1e-9);
        assert_eq!(run("clamp(15, 0, 10) + step(3, 2) + smoothstep(5, 0, 10)", 1.0), 10.0 + 1.0 + 0.5);
        assert_eq!(run("saw(5, 4, 8)", 1.0), 2.0);
        assert_eq!(run("v0 + (v1 - v0) * t", 1.0), 15.0);
        assert_eq!(run("rotation + rotation(2)", 3.0), 50.0, "other properties, now and at a frame");
        assert_eq!(run("cycle(4)", 10.0), 6.0);
        // random: deterministic per frame and seed, inside the range
        let r = run("random(5, 10)", 7.0);
        assert!((5.0..10.0).contains(&r));
        assert_eq!(r, run("random(5, 10)", 7.0));
        assert_ne!(run("random_s(1)", 7.0), run("random_s(2)", 7.0));
        let p = (run("random_p(4)", 8.0), run("random_p(4)", 9.0));
        assert!((p.0 - p.1).abs() < 0.5, "periodic random interpolates");
    }

    #[test]
    fn errors_are_reported() {
        for (src, msg) in [("1 +", "end"), ("foo + 1", "unknown name"), ("sin(1, 2)", "arguments"), ("bogus(1)", "unknown function"), ("(1", "')'"), ("1 2", "after")] {
            let e = Expr::parse(src, &[]).unwrap_err();
            assert!(e.contains(msg), "{src}: {e}");
        }
    }
}
