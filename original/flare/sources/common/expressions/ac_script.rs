//! Flare scripting.
//!
//! Two kinds of script, deliberately kept apart:
//!
//! 1. **Frame scripts** (`FramePlayer`) live *inside documents*, like Animate's
//!    Actions panel code. They are JavaScript with a CreateJS-style API
//!    (`this.stop()`, `this.gotoAndPlay("loop")`). Because they arrive with
//!    files from other people they run in a sandbox: embedded QuickJS, no file,
//!    network or editor access, a memory cap and a time limit per frame. They
//!    never run when a document is opened, only during playback.
//!
//! 2. **Authoring scripts** (`run_js`, `run_file`) are tools the *user* chooses
//!    to run, like Animate's JSFL commands. They drive the editor through the
//!    command registry (`ac_core::commands`), so every language gets the same
//!    API. JavaScript runs embedded; Python, Node, Ruby or any other language run
//!    as a separate process speaking a one-line JSON protocol over stdin/stdout
//!    (see `PROTOCOL`). They run with the user's permissions, so the app asks
//!    before running one.

use ac_core::{commands, Editor};
use rquickjs::{Context, Ctx, Function, Runtime};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Marker that prefixes protocol lines on an external script's stdout; any
/// other output is treated as the script's own printing.
pub const MARK: &str = "\u{1e}AC ";
pub const MAX_LINE: usize = 1 << 20;
pub const MAX_COMMANDS: usize = 200_000;

pub const PROTOCOL: &str = "\
Flare script protocol (any language)
-------------------------------------------
The script is started with its folder as the working directory and these
environment variables: ANIMATECRAFT=1, ANIMATECRAFT_HELPERS=<folder with
flare.py / flare.js / flare.rb>.

To call a command, write ONE line to stdout and flush:
    \\x1eAC {\"id\": 1, \"cmd\": \"draw.rect\", \"args\": {\"x\": 10, \"y\": 10, \"w\": 50, \"h\": 50}}
(\\x1e is the ASCII record-separator character, then 'AC ' and JSON.)
Read ONE line from stdin in reply:
    {\"id\": 1, \"result\": {...}}      or      {\"id\": 1, \"error\": \"message\"}
Anything else printed to stdout or stderr shows up in the Output panel.
";

#[derive(Debug, Default, Clone)]
pub struct ScriptReport {
    pub log: Vec<String>,
    pub error: Option<String>,
    pub commands: usize,
    pub elapsed: Duration,
}

impl ScriptReport {
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }
}

#[derive(Clone, Debug)]
pub struct ScriptOptions {
    pub timeout: Duration,
    /// Folder relative paths (exports) resolve against.
    pub base_dir: PathBuf,
    /// Interpreter candidates per extension, tried in order.
    pub interpreters: Vec<(String, Vec<Vec<String>>)>,
}

impl Default for ScriptOptions {
    fn default() -> Self {
        let py: Vec<Vec<String>> = if cfg!(windows) {
            vec![vec!["py".into(), "-3".into()], vec!["python".into()], vec!["python3".into()]]
        } else {
            vec![vec!["python3".into()], vec!["python".into()]]
        };
        ScriptOptions {
            timeout: Duration::from_secs(30),
            base_dir: std::env::current_dir().unwrap_or_default(),
            interpreters: vec![
                ("py".into(), py),
                ("mjs".into(), vec![vec!["node".into()]]),
                ("cjs".into(), vec![vec!["node".into()]]),
                ("rb".into(), vec![vec!["ruby".into()]]),
                ("lua".into(), vec![vec!["lua".into()], vec!["lua5.4".into()]]),
            ],
        }
    }
}

/// Languages offered in the UI. `Js` is embedded; the rest are external.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Js,
    /// JSFL scripts from the authoring tool, through the compatibility shim.
    Jsfl,
    Python,
    Node,
    Ruby,
}

impl Language {
    pub const ALL: [Language; 5] = [Language::Js, Language::Jsfl, Language::Python, Language::Node, Language::Ruby];
    pub fn name(self) -> &'static str {
        match self {
            Language::Js => "JavaScript (built-in)",
            Language::Jsfl => "JSFL (compatibility)",
            Language::Python => "Python",
            Language::Node => "JavaScript (Node.js)",
            Language::Ruby => "Ruby",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Language::Js => "js",
            Language::Jsfl => "jsfl",
            Language::Python => "py",
            Language::Node => "mjs",
            Language::Ruby => "rb",
        }
    }
    pub fn example(self) -> &'static str {
        match self {
            Language::Js => "// Draw a row of stars on a new layer, then tween them.\nac.layer.add({ name: \"Stars\" });\nfor (let i = 0; i < 5; i++) {\n  ac.draw.polystar({ cx: 60 + i * 100, cy: 200, r: 30, star: true, fill: \"#FFC41F\" });\n}\nac.timeline.insert_keyframe({ layer: \"Stars\", frame: 24 });\nconst n = ac.select.layer({ layer: \"Stars\" });\nac.selection.rotate({ degrees: 72 });\nac.timeline.set_tween({ kind: \"classic\", layer: \"Stars\", frame: 1 });\nprint(\"Stars:\", n);\n",
            Language::Jsfl => "// JSFL: scripts written for the authoring tool's JavaScript API.\nvar doc = fl.getDocumentDOM();\nvar tl = doc.getTimeline();\ntl.addNewLayer(\"Boxes\");\ndoc.setFillColor(\"#33AAFF\");\nfor (var i = 0; i < 4; i++) {\n  doc.addNewRectangle({ left: 40 + i * 90, top: 60, right: 110 + i * 90, bottom: 130 }, 8);\n}\ndoc.selectAll();\ndoc.convertToSymbol(\"movie clip\", \"Box Row\", \"center\");\ntl.insertKeyframe(23);\nfl.trace(\"layers: \" + tl.layerCount);\n",
            Language::Python => "import flare as ac\n\n# Draw a row of stars on a new layer, then tween them.\nac.layer.add(name=\"Stars\")\nfor i in range(5):\n    ac.draw.polystar(cx=60 + i * 100, cy=200, r=30, star=True, fill=\"#FFC41F\")\nac.timeline.insert_keyframe(layer=\"Stars\", frame=24)\nn = ac.select.layer(layer=\"Stars\")\nac.selection.rotate(degrees=72)\nac.timeline.set_tween(kind=\"classic\", layer=\"Stars\", frame=1)\nprint(\"Stars:\", n)\n",
            Language::Node => "import ac from \"./flare.mjs\";\n\nac.layer.add({ name: \"Stars\" });\nfor (let i = 0; i < 5; i++) {\n  ac.draw.polystar({ cx: 60 + i * 100, cy: 200, r: 30, star: true, fill: \"#FFC41F\" });\n}\nconsole.log(\"Layers:\", ac.layer.list().length);\n",
            Language::Ruby => "require \"flare\"\n\nac = Flare\nac.call(\"layer.add\", name: \"Stars\")\n5.times do |i|\n  ac.call(\"draw.polystar\", cx: 60 + i * 100, cy: 200, r: 30, star: true, fill: \"#FFC41F\")\nend\nputs \"Layers: #{ac.call('layer.list').length}\"\n",
        }
    }
}

// ------------------------------------------------------------------ commands (core + file output)

fn resolve(base: &Path, p: &str) -> PathBuf {
    let p = Path::new(p);
    if p.is_absolute() { p.to_path_buf() } else { base.join(p) }
}

/// Run a registry command, plus the file-output commands that need the renderer.
pub fn exec(ed: &mut Editor, base: &Path, id: &str, args: &Value) -> Result<Value, String> {
    let path = || args.get("path").and_then(Value::as_str).map(|p| resolve(base, p)).ok_or_else(|| "missing 'path'".to_string());
    let frame = || args.get("frame").and_then(Value::as_u64).map(|f| f.saturating_sub(1) as u32).unwrap_or(ed.frame);
    let scale = args.get("scale").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.05, 8.0) as f32;
    match id {
        "export.gif" => {
            let p = path()?;
            ac_render::export_gif(&ed.doc, scale, &p)?;
            Ok(json!(p.display().to_string()))
        }
        "export.png" => {
            let p = path()?;
            ac_render::export_png(&ed.doc, frame(), scale, &p)?;
            Ok(json!(p.display().to_string()))
        }
        "export.svg" => {
            let p = path()?;
            std::fs::write(&p, ac_render::export_svg(&ed.doc, frame())).map_err(|e| e.to_string())?;
            Ok(json!(p.display().to_string()))
        }
        // FLfile (JSFL): files as text; paths relative to the script
        "file.read" => std::fs::read_to_string(path()?).map(Value::from).map_err(|e| e.to_string()),
        "file.write" => {
            let p = path()?;
            let text = args.get("text").and_then(Value::as_str).unwrap_or("");
            let r = if args.get("append").and_then(Value::as_bool).unwrap_or(false) {
                use std::io::Write;
                std::fs::OpenOptions::new().create(true).append(true).open(&p).and_then(|mut f| f.write_all(text.as_bytes()))
            } else {
                std::fs::write(&p, text)
            };
            r.map(|_| Value::Bool(true)).map_err(|e| e.to_string())
        }
        "file.exists" => Ok(Value::Bool(path()?.exists())),
        "file.list" => {
            let p = path()?;
            let want = args.get("kind").and_then(Value::as_str).unwrap_or("all");
            let mut v: Vec<String> = std::fs::read_dir(&p)
                .map_err(|e| e.to_string())?
                .filter_map(|e| e.ok())
                .filter(|e| match want {
                    "files" => e.path().is_file(),
                    "directories" => e.path().is_dir(),
                    _ => true,
                })
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            v.sort();
            Ok(json!(v))
        }
        "file.mkdir" => std::fs::create_dir_all(path()?).map(|_| Value::Bool(true)).map_err(|e| e.to_string()),
        "doc.save" => {
            let p = path()?;
            let doc = ed.saved_doc();
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("acx")) {
                ac_package::save(&doc, &p, None)?;
            } else {
                std::fs::write(&p, doc.to_json()).map_err(|e| e.to_string())?;
            }
            Ok(json!(p.display().to_string()))
        }
        _ => commands::execute(ed, id, args),
    }
}

pub const EXTRA_COMMANDS: &[(&str, &str, &str)] = &[
    ("export.gif", "path, scale?", "Export the animation as a GIF"),
    ("export.png", "path, frame?, scale?", "Export one frame as PNG"),
    ("export.svg", "path, frame?", "Export one frame as SVG"),
    ("file.read", "path", "Read a text file (relative to the script)"),
    ("file.write", "path, text, append?", "Write (or append to) a text file"),
    ("file.exists", "path", "Does the file or folder exist"),
    ("file.list", "path, kind? ('files'|'directories'|'all')", "Names in a folder"),
    ("file.mkdir", "path", "Create a folder (and its parents)"),
    ("doc.save", "path", "Save the document (.acx package, or .acx.json for plain JSON)"),
];

pub fn all_command_ids() -> Vec<&'static str> {
    commands::COMMANDS.iter().map(|c| c.id).chain(EXTRA_COMMANDS.iter().map(|c| c.0)).collect()
}

pub fn reference() -> String {
    let mut s = commands::reference();
    for (id, args, summary) in EXTRA_COMMANDS {
        s += &format!("{id:<32} {args:<60} {summary}\n");
    }
    s
}

// ------------------------------------------------------------------ QuickJS helpers

fn js_error(ctx: &Ctx, e: rquickjs::Error) -> String {
    if let rquickjs::Error::Exception = e {
        let v = ctx.catch();
        if let Some(ex) = v.as_exception() {
            let kind: String = ex.get("name").unwrap_or_else(|_| "Error".into());
            let msg = format!("{kind}: {}", ex.message().unwrap_or_default());
            let stack = ex.stack().unwrap_or_default();
            return if stack.trim().is_empty() { msg } else { format!("{msg}\n{}", stack.trim_end()) };
        }
        if let Ok(s) = ctx.json_stringify(v) {
            if let Some(s) = s.and_then(|s| s.to_string().ok()) {
                return format!("uncaught: {s}");
            }
        }
        return "uncaught exception".into();
    }
    e.to_string()
}

fn sandbox_runtime(memory: usize, deadline: Rc<RefCell<Instant>>) -> Result<(Runtime, Context), String> {
    let rt = Runtime::new().map_err(|e| e.to_string())?;
    rt.set_memory_limit(memory);
    rt.set_max_stack_size(512 * 1024);
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > *deadline.borrow())));
    let ctx = Context::full(&rt).map_err(|e| e.to_string())?;
    Ok((rt, ctx))
}

const LOG_PRELUDE: &str = r##"
var print = (...a) => __ac_log(a.map(x => typeof x === "string" ? x : JSON.stringify(x)).join(" "));
var console = { log: print, info: print, warn: print, error: print };
var trace = print;
"##;

// ------------------------------------------------------------------ authoring: embedded JS

const AUTHORING_PRELUDE: &str = r##"
var ac = (() => {
  const call = (id, args) => {
    const r = JSON.parse(__ac_call(id, JSON.stringify(args === undefined ? {} : args)));
    if (r.error !== undefined) throw new Error(id + ": " + r.error);
    return r.result;
  };
  const api = { call };
  for (const id of JSON.parse(__ac_ids)) {
    const parts = id.split(".");
    let o = api;
    for (let i = 0; i < parts.length - 1; i++) o = (o[parts[i]] = o[parts[i]] || {});
    o[parts[parts.length - 1]] = (args) => call(id, args);
  }
  return api;
})();
"##;

/// The JSFL compatibility layer (`fl`, Document, Timeline, Library…).
pub const JSFL_PRELUDE: &str = include_str!("jsfl.js");

/// Run a JSFL script: the shim, then the script (one undo step).
pub fn run_jsfl(ed: &mut Editor, code: &str, opts: &ScriptOptions) -> ScriptReport {
    // keep the script's own line numbers in errors: the shim is one
    // (global) eval in front of the script's first line
    let shim = serde_json::to_string(JSFL_PRELUDE).unwrap();
    run_js(ed, &format!("(0, eval)({shim}); {code}"), opts)
}

/// Run JavaScript against the editor (one undo step for the whole run).
pub fn run_js(ed: &mut Editor, code: &str, opts: &ScriptOptions) -> ScriptReport {
    let start = Instant::now();
    let mark = ed.undo_depth();
    // The JS host functions need shared access to the editor while the script runs.
    let shared = Rc::new(RefCell::new(std::mem::replace(ed, Editor::new(Default::default()))));
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let count = Rc::new(RefCell::new(0usize));
    let deadline = Rc::new(RefCell::new(start + opts.timeout));
    let result: Result<(), String> = (|| {
        let (rt, ctx) = sandbox_runtime(256 << 20, deadline.clone())?;
        let r = ctx.with(|ctx| -> Result<(), String> {
            let g = ctx.globals();
            let l = log.clone();
            g.set("__ac_log", Function::new(ctx.clone(), move |s: String| l.borrow_mut().push(s)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let (sh, c, base) = (shared.clone(), count.clone(), opts.base_dir.clone());
            let call = move |id: String, args: String| -> String {
                *c.borrow_mut() += 1;
                let args: Value = serde_json::from_str(&args).unwrap_or(Value::Null);
                match exec(&mut sh.borrow_mut(), &base, &id, &args) {
                    Ok(v) => json!({ "result": v }).to_string(),
                    Err(e) => json!({ "error": e }).to_string(),
                }
            };
            g.set("__ac_call", Function::new(ctx.clone(), call).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            g.set("__ac_ids", serde_json::to_string(&all_command_ids()).unwrap()).map_err(|e| e.to_string())?;
            ctx.eval::<(), _>(LOG_PRELUDE).map_err(|e| js_error(&ctx, e))?;
            ctx.eval::<(), _>(AUTHORING_PRELUDE).map_err(|e| js_error(&ctx, e))?;
            ctx.eval::<(), _>(code).map_err(|e| js_error(&ctx, e))
        });
        drop(ctx);
        drop(rt);
        r
    })();
    let timed_out = Instant::now() > *deadline.borrow();
    *ed = Rc::try_unwrap(shared).ok().expect("script engine released the editor").into_inner();
    finish(ed, mark, "Run Script");
    let commands = *count.borrow();
    let log = log.borrow().clone();
    ScriptReport {
        log,
        error: result.err().map(|e| if timed_out { format!("stopped: script ran longer than {:?}", opts.timeout) } else { e }),
        commands,
        elapsed: start.elapsed(),
    }
}

fn finish(ed: &mut Editor, mark: usize, label: &str) {
    if ed.undo_depth() > mark {
        ed.squash_undo(mark, label);
    }
}

// ------------------------------------------------------------------ authoring: external processes

const PY_HELPER: &str = r##""""Flare scripting helper for Python.

    import flare as ac
    ac.draw.rect(x=10, y=10, w=100, h=50, fill="#FF0000")
    info = ac.doc.info()

Every command in Commands > Command Reference is available as ac.<group>.<name>(**args),
or as ac.call("group.name", {...}).
"""
import json as _json
import sys as _sys

_MARK = "\x1eAC "
_n = 0
try:
    _sys.stdout.reconfigure(encoding="utf-8")
except Exception:
    pass


class FlareError(RuntimeError):
    pass


def call(cmd, args=None, **kw):
    global _n
    _n += 1
    payload = dict(args or {}, **kw)
    _sys.stdout.write(_MARK + _json.dumps({"id": _n, "cmd": cmd, "args": payload}) + "\n")
    _sys.stdout.flush()
    line = _sys.stdin.readline()
    if not line:
        raise FlareError("Flare closed the connection")
    reply = _json.loads(line)
    if "error" in reply:
        raise FlareError(cmd + ": " + reply["error"])
    return reply.get("result")


class _Group:
    def __init__(self, path):
        self._path = path

    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        return _Group(self._path + "." + name)

    def __call__(self, args=None, **kw):
        return call(self._path, args, **kw)


def __getattr__(name):  # module-level: ac.draw, ac.layer, ...
    if name.startswith("__"):
        raise AttributeError(name)
    return _Group(name)
"##;

const NODE_HELPER: &str = r##"// Flare scripting helper for Node.js (ES module).
//   import ac from "./flare.mjs";   // copied next to your script while it runs
//   ac.draw.rect({ x: 10, y: 10, w: 100, h: 50 });
import fs from "node:fs";
const MARK = "\x1eAC ";
let n = 0, pending = "";
function readLine() {
  const buf = Buffer.alloc(65536);
  for (;;) {
    const i = pending.indexOf("\n");
    if (i >= 0) { const line = pending.slice(0, i); pending = pending.slice(i + 1); return line; }
    let read = 0;
    try { read = fs.readSync(0, buf, 0, buf.length, null); }
    catch (e) { if (e.code === "EAGAIN") continue; throw e; }
    if (read === 0) throw new Error("Flare closed the connection");
    pending += buf.toString("utf8", 0, read);
  }
}
export function call(cmd, args = {}) {
  n += 1;
  fs.writeSync(1, MARK + JSON.stringify({ id: n, cmd, args }) + "\n");
  const reply = JSON.parse(readLine());
  if ("error" in reply) throw new Error(cmd + ": " + reply.error);
  return reply.result;
}
const group = (path) => new Proxy(function () {}, {
  get: (_, k) => (k === "then" ? undefined : group(path ? path + "." + String(k) : String(k))),
  apply: (_, __, a) => call(path, a[0] || {}),
});
const ac = new Proxy({ call }, { get: (t, k) => (k in t ? t[k] : group(String(k))) });
export default ac;
"##;

const RUBY_HELPER: &str = r##"# Flare scripting helper for Ruby.
#   require "flare"
#   Flare.call("draw.rect", x: 10, y: 10, w: 100, h: 50)
require "json"
$stdout.sync = true
module Flare
  MARK = "\x1eAC "
  @n = 0
  def self.call(cmd, args = {})
    @n += 1
    $stdout.write(MARK + JSON.generate({ id: @n, cmd: cmd, args: args }) + "\n")
    line = $stdin.gets or raise "Flare closed the connection"
    reply = JSON.parse(line)
    raise "#{cmd}: #{reply['error']}" if reply.key?("error")
    reply["result"]
  end
end
"##;

const PY_LEGACY: &str = "# Old name of the Flare helper (the app was called AnimationCraft).\nimport sys as _sys\nimport flare as _flare\n_flare.AnimationCraftError = _flare.FlareError\n_sys.modules[__name__] = _flare\n";
const NODE_LEGACY: &str = "// Old name of the Flare helper (the app was called AnimationCraft).\nexport { default } from \"./flare.mjs\";\nexport * from \"./flare.mjs\";\n";
const RUBY_LEGACY: &str = "# Old name of the Flare helper (the app was called AnimationCraft).\nrequire_relative \"flare\"\nAnimationCraft = Flare\n";

/// Write the language helpers into a folder and return it.
pub fn helper_dir() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("flare-helpers-{}", env!("CARGO_PKG_VERSION")));
    std::fs::create_dir_all(&dir)?;
    // the old module names (from when the app was called AnimationCraft) still work
    for (name, body) in [("flare.py", PY_HELPER), ("flare.mjs", NODE_HELPER), ("flare.rb", RUBY_HELPER), ("animationcraft.py", PY_LEGACY), ("animationcraft.mjs", NODE_LEGACY), ("animationcraft.rb", RUBY_LEGACY)] {
        let p = dir.join(name);
        if std::fs::read_to_string(&p).map(|s| s != body).unwrap_or(true) {
            std::fs::write(&p, body)?;
        }
    }
    std::fs::write(dir.join("PROTOCOL.txt"), PROTOCOL)?;
    Ok(dir)
}

enum Msg {
    Out(String),
    Err(String),
    Closed,
}

/// Run a script file. `.js` runs embedded; other extensions use the
/// interpreter table (`.py`, `.mjs`, `.rb`, `.lua` by default).
pub fn run_file(ed: &mut Editor, path: &Path, opts: &ScriptOptions) -> ScriptReport {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut opts = opts.clone();
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        opts.base_dir = dir.to_path_buf();
    }
    if ext == "js" || ext == "jsfl" {
        return match std::fs::read_to_string(path) {
            Ok(code) if ext == "jsfl" => run_jsfl(ed, &code, &opts),
            Ok(code) => run_js(ed, &code, &opts),
            Err(e) => ScriptReport { error: Some(format!("{}: {e}", path.display())), ..Default::default() },
        };
    }
    let Some((_, candidates)) = opts.interpreters.iter().find(|(e, _)| *e == ext) else {
        return ScriptReport { error: Some(format!("no interpreter configured for .{ext} files")), ..Default::default() };
    };
    run_external(ed, path, &candidates.clone(), &opts)
}

/// Run `code` written in `lang` (the Script Console). External languages are
/// written to a temporary file first.
pub fn run_code(ed: &mut Editor, lang: Language, code: &str, opts: &ScriptOptions) -> ScriptReport {
    if lang == Language::Js {
        return run_js(ed, code, opts);
    }
    if lang == Language::Jsfl {
        return run_jsfl(ed, code, opts);
    }
    let dir = match helper_dir() {
        Ok(d) => d,
        Err(e) => return ScriptReport { error: Some(format!("can't write helper files: {e}")), ..Default::default() },
    };
    let file = dir.join(format!("console-script.{}", lang.ext()));
    if let Err(e) = std::fs::write(&file, code) {
        return ScriptReport { error: Some(e.to_string()), ..Default::default() };
    }
    let mut o = opts.clone();
    let base = o.base_dir.clone();
    let candidates = o.interpreters.iter().find(|(e, _)| e == lang.ext()).map(|x| x.1.clone()).unwrap_or_default();
    o.base_dir = base;
    run_external_in(ed, &file, &candidates, &o, &o.base_dir.clone())
}

pub fn run_external(ed: &mut Editor, script: &Path, candidates: &[Vec<String>], opts: &ScriptOptions) -> ScriptReport {
    let cwd = opts.base_dir.clone();
    run_external_in(ed, script, candidates, opts, &cwd)
}

fn run_external_in(ed: &mut Editor, script: &Path, candidates: &[Vec<String>], opts: &ScriptOptions, cwd: &Path) -> ScriptReport {
    let start = Instant::now();
    let mut rep = ScriptReport::default();
    let helpers = match helper_dir() {
        Ok(d) => d,
        Err(e) => {
            rep.error = Some(format!("can't write helper files: {e}"));
            return rep;
        }
    };
    // Node resolves relative imports from the script, so put the helper beside it too.
    if script.extension().map(|e| e == "mjs").unwrap_or(false) {
        if let Some(dir) = script.parent() {
            let _ = std::fs::write(dir.join("flare.mjs"), NODE_HELPER);
            if std::fs::read_to_string(script).is_ok_and(|t| t.contains("animationcraft.mjs")) {
                let _ = std::fs::write(dir.join("animationcraft.mjs"), NODE_LEGACY);
            }
        }
    }
    let script_abs = std::fs::canonicalize(script).unwrap_or_else(|_| script.to_path_buf());
    let sep = if cfg!(windows) { ";" } else { ":" };
    let join = |var: &str| match std::env::var(var) {
        Ok(v) if !v.is_empty() => format!("{}{sep}{v}", helpers.display()),
        _ => helpers.display().to_string(),
    };
    let mut child = None;
    let mut tried = Vec::new();
    for cand in candidates {
        let Some((prog, pre)) = cand.split_first() else { continue };
        tried.push(prog.clone());
        let spawned = Command::new(prog)
            .args(pre)
            .arg(&script_abs)
            .current_dir(cwd)
            .env("ANIMATECRAFT", "1")
            .env("ANIMATECRAFT_HELPERS", &helpers)
            .env("PYTHONPATH", join("PYTHONPATH"))
            .env("PYTHONUNBUFFERED", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .env("RUBYLIB", join("RUBYLIB"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        if let Ok(c) = spawned {
            child = Some(c);
            break;
        }
    }
    let Some(mut child) = child else {
        rep.error = Some(format!(
            "couldn't start an interpreter for {} (tried: {}). Install it, or check it is on your PATH.",
            script.display(),
            tried.join(", ")
        ));
        return rep;
    };
    let mut stdin = child.stdin.take().unwrap();
    let (tx, rx) = mpsc::channel::<Msg>();
    let out = child.stdout.take().unwrap();
    let tx_out = tx.clone();
    std::thread::spawn(move || {
        let mut r = BufReader::new(out);
        loop {
            let mut buf = Vec::new();
            match (&mut r).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
                    if tx_out.send(Msg::Out(line)).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = tx_out.send(Msg::Closed);
    });
    let err = child.stderr.take().unwrap();
    let tx_err = tx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            let _ = tx_err.send(Msg::Err(line));
        }
    });
    drop(tx);

    let mark = ed.undo_depth();
    let deadline = start + opts.timeout;
    let mut closed = false;
    while !closed {
        let now = Instant::now();
        if now >= deadline {
            let _ = child.kill();
            rep.error = Some(format!("stopped: script ran longer than {:?}", opts.timeout));
            break;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(Msg::Out(line)) => {
                if let Some(req) = line.strip_prefix(MARK) {
                    if rep.commands >= MAX_COMMANDS {
                        let _ = child.kill();
                        rep.error = Some(format!("stopped: more than {MAX_COMMANDS} commands"));
                        break;
                    }
                    rep.commands += 1;
                    let reply = match serde_json::from_str::<Value>(req) {
                        Ok(v) => {
                            let id = v.get("id").cloned().unwrap_or(Value::Null);
                            let cmd = v.get("cmd").and_then(Value::as_str).unwrap_or("");
                            let args = v.get("args").cloned().unwrap_or(Value::Null);
                            match exec(ed, &opts.base_dir, cmd, &args) {
                                Ok(r) => json!({ "id": id, "result": r }),
                                Err(e) => json!({ "id": id, "error": e }),
                            }
                        }
                        Err(e) => json!({ "id": null, "error": format!("bad request: {e}") }),
                    };
                    if writeln!(stdin, "{reply}").and_then(|_| stdin.flush()).is_err() {
                        break;
                    }
                } else {
                    rep.log.push(line);
                }
            }
            Ok(Msg::Err(line)) => rep.log.push(line),
            Ok(Msg::Closed) => closed = true,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => closed = true,
        }
    }
    drop(stdin);
    let status = child.wait().ok();
    // Drain whatever stderr is left (tracebacks arrive last).
    while let Ok(m) = rx.recv_timeout(Duration::from_millis(200)) {
        match m {
            Msg::Out(l) | Msg::Err(l) => rep.log.push(l),
            Msg::Closed => {}
        }
    }
    if rep.error.is_none() {
        if let Some(s) = status.filter(|s| !s.success()) {
            rep.error = Some(format!("script exited with {s}"));
        }
    }
    finish(ed, mark, "Run Script");
    rep.elapsed = start.elapsed();
    rep
}

// ------------------------------------------------------------------ frame scripts (sandboxed)

/// What a frame script asked the playhead to do.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayAction {
    Stop,
    Play,
    /// 0-based frame (CreateJS / Animate HTML5 Canvas numbering).
    Goto { frame: u32, play: bool },
}

/// A named instance as frame scripts see it (`this.ball.x`), CreateJS-style.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub name: String,
    pub x: f32,
    pub y: f32,
    /// degrees
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub alpha: f32,
    pub visible: bool,
    /// Text fields: their text (`this.score.text = "10"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

const FRAME_PRELUDE: &str = r##"
var __tl = {
  stop() { __ac_act("stop", ""); },
  play() { __ac_act("play", ""); },
  gotoAndPlay(f) { __ac_act("goto_play", String(f)); },
  gotoAndStop(f) { __ac_act("goto_stop", String(f)); },
  nextFrame() { __ac_act("goto_stop", String(this.currentFrame + 1)); },
  prevFrame() { __ac_act("goto_stop", String(Math.max(0, this.currentFrame - 1))); },
};
__tl.getChildByName = function(n) { return this[n] || null; };
var __ac_before = [];
function __ac_bind(json) {
  __ac_before = JSON.parse(json);
  // mouse events only fire in published HTML5 pages; accept them quietly here
  const quiet = { addEventListener() {}, removeEventListener() {}, on() {}, off() {}, removeAllEventListeners() {} };
  for (const o of __ac_before) { __tl[o.name] = Object.assign(Object.create(quiet), o); }
}
function __ac_dump() {
  return JSON.stringify(__ac_before.map(o => {
    const c = __tl[o.name];
    if (!c) return o;
    const r = { name: o.name, x: +c.x, y: +c.y, rotation: +c.rotation, scaleX: +c.scaleX, scaleY: +c.scaleY, alpha: +c.alpha, visible: !!c.visible };
    if (o.text !== undefined) r.text = c.text === undefined || c.text === null ? "" : String(c.text);
    return r;
  }));
}
var stop = () => __tl.stop(), play = () => __tl.play();
var gotoAndPlay = (f) => __tl.gotoAndPlay(f), gotoAndStop = (f) => __tl.gotoAndStop(f);
"##;

/// Runs document frame scripts during playback. One instance per playback
/// session, so variables persist from frame to frame like in a published movie.
pub struct FramePlayer {
    rt: Runtime,
    ctx: Context,
    actions: Rc<RefCell<Vec<(String, String)>>>,
    pub log: Rc<RefCell<Vec<String>>>,
    deadline: Rc<RefCell<Instant>>,
    pub frame_budget: Duration,
}

impl FramePlayer {
    pub fn new() -> Result<FramePlayer, String> {
        let deadline = Rc::new(RefCell::new(Instant::now()));
        let (rt, ctx) = sandbox_runtime(32 << 20, deadline.clone())?;
        let actions = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::new(RefCell::new(Vec::new()));
        ctx.with(|ctx| -> Result<(), String> {
            let g = ctx.globals();
            let a = actions.clone();
            g.set("__ac_act", Function::new(ctx.clone(), move |k: String, v: String| a.borrow_mut().push((k, v))).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let l = log.clone();
            g.set("__ac_log", Function::new(ctx.clone(), move |s: String| l.borrow_mut().push(s)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            *deadline.borrow_mut() = Instant::now() + Duration::from_secs(1);
            ctx.eval::<(), _>(LOG_PRELUDE).map_err(|e| js_error(&ctx, e))?;
            ctx.eval::<(), _>(FRAME_PRELUDE).map_err(|e| js_error(&ctx, e))
        })?;
        Ok(FramePlayer { rt, ctx, actions, log, deadline, frame_budget: Duration::from_millis(100) })
    }

    /// Run one frame's script. `frame` is 0-based; `labels` maps frame labels
    /// to 0-based frames. Returns the playhead actions the script requested.
    pub fn run(&mut self, code: &str, frame: u32, total: u32, labels: &[(String, u32)]) -> Result<Vec<PlayAction>, String> {
        self.run_with(code, frame, total, labels, &[]).map(|r| r.0)
    }

    /// Like `run`, exposing named instances as `this.<name>` (x, y, rotation,
    /// scaleX, scaleY, alpha, visible). Returns the instances the script changed.
    pub fn run_with(&mut self, code: &str, frame: u32, total: u32, labels: &[(String, u32)], instances: &[Instance]) -> Result<(Vec<PlayAction>, Vec<Instance>), String> {
        let json = serde_json::to_string(instances).map_err(|e| e.to_string())?;
        self.ctx.with(|ctx| -> Result<(), String> {
            let f: Function = ctx.globals().get("__ac_bind").map_err(|e| e.to_string())?;
            f.call::<_, ()>((json,)).map_err(|e| js_error(&ctx, e))
        })?;
        let actions = self.run_inner(code, frame, total, labels)?;
        let after: String = self.ctx.with(|ctx| -> Result<String, String> {
            let f: Function = ctx.globals().get("__ac_dump").map_err(|e| e.to_string())?;
            f.call::<_, String>(()).map_err(|e| js_error(&ctx, e))
        })?;
        let after: Vec<Instance> = serde_json::from_str(&after).map_err(|e| format!("instance values must be numbers: {e}"))?;
        let changed = after.into_iter().zip(instances).filter(|(a, b)| a != *b).map(|(a, _)| a).collect();
        Ok((actions, changed))
    }

    fn run_inner(&mut self, code: &str, frame: u32, total: u32, labels: &[(String, u32)]) -> Result<Vec<PlayAction>, String> {
        self.actions.borrow_mut().clear();
        *self.deadline.borrow_mut() = Instant::now() + self.frame_budget;
        let budget = self.frame_budget;
        let res = self.ctx.with(|ctx| -> Result<(), String> {
            ctx.eval::<(), _>(format!("__tl.currentFrame = {frame}; __tl.totalFrames = {total};")).map_err(|e| js_error(&ctx, e))?;
            // Run as a function body with `this` = the timeline, as Animate does.
            let wrapped = format!("(function() {{\n{code}\n}}).call(__tl);");
            ctx.eval::<(), _>(wrapped).map_err(|e| js_error(&ctx, e))
        });
        if let Err(e) = res {
            let timed_out = Instant::now() > *self.deadline.borrow();
            return Err(if timed_out { format!("frame script stopped after {budget:?} (infinite loop?)") } else { e });
        }
        let mut out = Vec::new();
        for (k, v) in self.actions.borrow().iter() {
            let target = || -> Result<u32, String> {
                if let Ok(n) = v.parse::<f64>() {
                    return Ok((n.max(0.0) as u32).min(total.saturating_sub(1)));
                }
                labels.iter().find(|(l, _)| l == v).map(|(_, f)| *f).ok_or_else(|| format!("no frame label \"{v}\""))
            };
            out.push(match k.as_str() {
                "stop" => PlayAction::Stop,
                "play" => PlayAction::Play,
                "goto_play" => PlayAction::Goto { frame: target()?, play: true },
                _ => PlayAction::Goto { frame: target()?, play: false },
            });
        }
        Ok(out)
    }

    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }

    #[doc(hidden)]
    pub fn runtime(&self) -> &Runtime {
        &self.rt
    }
}

/// Frame labels of a timeline (first label wins), for `gotoAndPlay("label")`.
pub fn labels(tl: &ac_core::Timeline) -> Vec<(String, u32)> {
    let mut v: Vec<(String, u32)> = Vec::new();
    for l in &tl.layers {
        for (f, k) in &l.keyframes {
            if let Some(name) = k.target_label() {
                if !v.iter().any(|(n, _)| n == name) {
                    v.push((name.to_string(), *f));
                }
            }
        }
    }
    v
}

/// Scripts on keyframes that start exactly at `frame` (top layer first).
pub fn scripts_at(tl: &ac_core::Timeline, frame: u32) -> Vec<String> {
    tl.layers.iter().filter_map(|l| l.keyframes.get(&frame).and_then(|k| k.script.clone())).filter(|s| !s.trim().is_empty()).collect()
}

#[cfg(test)]
#[path = "ac_script_tests.rs"]
mod tests;
