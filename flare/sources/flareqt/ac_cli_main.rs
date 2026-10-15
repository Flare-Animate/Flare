//! flare-cli — headless access to the same engine the editor uses.
//!
//!   flare-cli info   <doc.acx.json | demo>
//!   flare-cli gif    <doc | demo> <out.gif> [scale]
//!   flare-cli png    <doc | demo> <frame> <out.png|.jpg|.gif> [scale] [--quality N] [--colors N] [--transparent]
//!   flare-cli svg    <doc | demo> <frame> <out.svg>
//!   flare-cli png-seq <doc | demo> <out-dir> [scale]
//!   flare-cli video  <doc | demo> <out.mp4|.webm|.mov> [scale]   (needs ffmpeg)
//!   flare-cli xfl    <doc | demo> <out.fla | out.xfl>   (export; any <doc> may also be a .fla / .xfl / XFL folder)
//!   flare-cli html5  <doc | demo> <out-dir | out.html> [--no-loop]   (HTML5 Canvas; .html = one self-contained file)
//!   flare-cli sprites <doc | demo> <out(.png)> [scale] [--format hash|array|starling]   (sprite sheet + atlas)
//!   flare-cli svg-anim <doc | demo> <out.svg>   (animated SVG, SMIL)
//!   flare-cli lottie <doc | demo> <out.json>
//!   flare-cli convert <in> <out.acx.json | .fla | .xfl>
//!   flare-cli save-demo <out.acx.json>
//!   flare-cli run <script.js|.py|.mjs|.rb> [--doc <doc|demo>] [--save out.acx.json] [--gif out.gif]
//!   flare-cli commands            (list scriptable commands)
//!   flare-cli effects [search]    (EffectCraft effects usable as object filters)
//!   flare-cli xdts <doc> <out.xdts> (XDTS timing sheet)

use ac_core::{demo::demo_document, Document, ElementKind, Tween};
use std::path::Path;

fn load(src: &str) -> Result<Document, String> {
    if src == "demo" {
        return Ok(demo_document());
    }
    let p = Path::new(src);
    if p.is_dir() || [".fla", ".xfl"].iter().any(|e| src.to_ascii_lowercase().ends_with(e)) {
        let im = ac_xfl::open(p)?;
        for w in &im.warnings {
            eprintln!("note: {w}");
        }
        return Ok(im.doc);
    }
    if src.to_ascii_lowercase().ends_with(".acx") {
        let l = ac_package::open(p)?;
        for w in &l.warnings {
            eprintln!("note: {w}");
        }
        return Ok(l.doc);
    }
    let s = std::fs::read_to_string(src).map_err(|e| format!("{src}: {e}"))?;
    Document::from_json(&s).map_err(|e| format!("{src}: {e}"))
}

/// Save as .acx (package), .acx.json (plain JSON), or .fla / .xfl by extension.
fn save_any(d: &Document, out: &str) -> Result<(), String> {
    let low = out.to_ascii_lowercase();
    if low.ends_with(".fla") || low.ends_with(".xfl") {
        for w in ac_xfl::save(&d.with_floats_committed(), Path::new(out))? {
            eprintln!("note: {w}");
        }
        Ok(())
    } else if low.ends_with(".acx") {
        let thumb = ac_render::render_frame(d, 0, (256.0 / d.width.max(1.0)).min(1.0)).encode_png().ok();
        ac_package::save(d, Path::new(out), thumb.as_deref())
    } else {
        std::fs::write(out, d.with_floats_committed().to_json()).map_err(|e| e.to_string())
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let arg = |i: usize| args.get(i).map(String::as_str).ok_or_else(|| format!("missing argument {i}; run with no args for usage"));
    match arg(0)? {
        "info" => {
            let d = load(arg(1)?)?;
            println!("Stage {}x{} @ {} fps, {} frames", d.width, d.height, d.fps, d.total_frames());
            println!("Layers (top first):");
            for l in &d.scene.layers {
                let kfs: Vec<String> = l.keyframes.iter().map(|(f, k)| {
                    let t = match k.tween { Tween::None => "", Tween::Classic { .. } => " classic→", Tween::Shape { .. } => " shape→", Tween::Motion { .. } => " motion" };
                    format!("{}{}", f + 1, t)
                }).collect();
                println!("  {:<8} frames 1-{:<3} keyframes [{}]", l.name, l.end, kfs.join(", "));
            }
            println!("Library:");
            for s in &d.library {
                let insts: usize = s.timeline.layers.iter().flat_map(|l| l.keyframes.values()).flat_map(|k| &k.elements).filter(|e| matches!(e.kind, ElementKind::Instance(_))).count();
                println!("  {:<10} {:?}, {} frames, {} layers, {} nested instances", s.name, s.kind, s.timeline.frame_count(), s.timeline.layers.len(), insts);
            }
        }
        "gif" => {
            let d = load(arg(1)?)?;
            let scale = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            ac_render::export_gif(&d, scale, Path::new(arg(2)?))?;
            println!("wrote {}", arg(2)?);
        }
        "png-seq" => {
            let d = load(arg(1)?)?;
            let o = ac_render::ExportOpts { scale: args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0), ..Default::default() };
            let n = ac_render::export_png_sequence(&d, &o, Path::new(arg(2)?), "frame", &mut |_, _| true)?;
            println!("wrote {n} frames to {}", arg(2)?);
        }
        "video" => {
            let d = load(arg(1)?)?;
            let o = ac_render::ExportOpts { scale: args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0), ..Default::default() };
            ac_render::export_video(&d, &o, Path::new(arg(2)?), "ffmpeg", &mut |a, b| { eprint!("\rframe {a}/{b}"); true })?;
            eprintln!();
            println!("wrote {}", arg(2)?);
        }
        "png" | "image" => {
            // png <doc> <frame> <out.png|.jpg|.gif> [scale] [--quality N] [--colors N] [--transparent]
            let d = load(arg(1)?)?;
            let f: u32 = arg(2)?.parse().map_err(|_| "frame must be a number (1-based)")?;
            let scale = args.get(4).filter(|s| !s.starts_with("--")).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u16>().ok());
            let transparent = args.iter().any(|a| a == "--transparent");
            let out = Path::new(arg(3)?);
            let fmt = match out.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
                Some("jpg") | Some("jpeg") => ac_render::StillFormat::Jpeg { quality: flag("--quality").unwrap_or(80).min(100) as u8 },
                Some("gif") => ac_render::StillFormat::Gif { colors: flag("--colors").unwrap_or(256), transparent },
                _ => ac_render::StillFormat::Png { transparent },
            };
            ac_render::export_still(&d, f.saturating_sub(1), scale, fmt, out)?;
            println!("wrote {}", arg(3)?);
        }
        "svg" => {
            let d = load(arg(1)?)?;
            let f: u32 = arg(2)?.parse().map_err(|_| "frame must be a number (1-based)")?;
            std::fs::write(arg(3)?, ac_render::export_svg(&d, f.saturating_sub(1))).map_err(|e| e.to_string())?;
            println!("wrote {}", arg(3)?);
        }
        "xfl" => {
            let d = load(arg(1)?)?;
            for w in ac_xfl::save(&d, Path::new(arg(2)?))? {
                eprintln!("note: {w}");
            }
            println!("wrote {}", arg(2)?);
        }
        "html5" => {
            let d = load(arg(1)?)?;
            let o = ac_render::html5::Html5Opts { loop_play: !args.iter().any(|a| a == "--no-loop"), ..Default::default() };
            let page = ac_render::html5::publish_to(&d, &o, Path::new(arg(2)?))?;
            println!("wrote {}", page.display());
        }
        "sprites" => {
            let d = load(arg(1)?)?;
            let o = ac_render::ExportOpts { scale: args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0), transparent: true, ..Default::default() };
            let fmt = args.iter().position(|a| a == "--format").and_then(|i| args.get(i + 1)).map(String::as_str).unwrap_or("hash");
            let so = ac_render::formats::SheetOpts {
                format: match fmt {
                    "array" => ac_render::formats::AtlasFormat::JsonArray,
                    "starling" | "xml" => ac_render::formats::AtlasFormat::StarlingXml,
                    _ => ac_render::formats::AtlasFormat::JsonHash,
                },
                ..Default::default()
            };
            let (a, b) = ac_render::formats::export_sprite_sheet(&d, &o, &so, Path::new(arg(2)?))?;
            println!("wrote {} and {}", a.display(), b.display());
        }
        "svg-anim" => {
            let d = load(arg(1)?)?;
            std::fs::write(arg(2)?, ac_render::export_animated_svg(&d, &ac_render::ExportOpts::default())).map_err(|e| e.to_string())?;
            println!("wrote {}", arg(2)?);
        }
        "lottie" => {
            let d = load(arg(1)?)?;
            let (json, warn) = ac_render::formats::lottie(&d, &ac_render::ExportOpts::default());
            for w in warn {
                eprintln!("note: {w}");
            }
            std::fs::write(arg(2)?, json).map_err(|e| e.to_string())?;
            println!("wrote {}", arg(2)?);
        }
        "xdts" => {
            // xdts <doc> <out.xdts>: timing sheet of the first scene
            let d = load(arg(1)?)?;
            std::fs::write(arg(2)?, ac_core::xdts::export(&d, &d.scene)).map_err(|e| e.to_string())?;
            println!("wrote {}", arg(2)?);
        }
        "convert" => {
            let d = load(arg(1)?)?;
            save_any(&d, arg(2)?)?;
            println!("wrote {}", arg(2)?);
        }
        "save-demo" => {
            save_any(&demo_document(), arg(1)?)?;
            println!("wrote {}", arg(1)?);
        }
        "effects" => {
            // effects [search]: the EffectCraft effects usable as object filters
            let q = args.get(1).map(|s| s.to_lowercase());
            for e in ac_render::effectcraft::catalog() {
                if q.as_ref().is_some_and(|q| !(e.id.to_lowercase().contains(q) || e.name.to_lowercase().contains(q) || e.category.to_lowercase().contains(q))) {
                    continue;
                }
                let ps: Vec<String> = e.params.iter().map(|p| p.id.to_string()).collect();
                println!("{:<18} {:<34} {:<32} {}", e.category, e.id, e.name, ps.join(", "));
            }
        }
        "commands" => {
            print!("{}", ac_script::reference());
            println!("\n{}", ac_script::PROTOCOL);
        }
        "run" => {
            let script = std::path::PathBuf::from(arg(1)?);
            let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
            let doc = match flag("--doc") {
                Some(d) => load(d)?,
                None => ac_core::Document::default(),
            };
            let mut ed = ac_core::Editor::new(doc);
            let rep = ac_script::run_file(&mut ed, &script, &ac_script::ScriptOptions::default());
            for line in &rep.log {
                println!("{line}");
            }
            eprintln!("[{} commands in {:.2?}]", rep.commands, rep.elapsed);
            if let Some(e) = rep.error {
                return Err(e);
            }
            if let Some(p) = flag("--save") {
                save_any(&ed.saved_doc(), p)?;
                eprintln!("saved {p}");
            }
            if let Some(p) = flag("--gif") {
                ac_render::export_gif(&ed.doc, 1.0, Path::new(p))?;
                eprintln!("wrote {p}");
            }
        }
        other => return Err(format!("unknown command '{other}'")),
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ac_audio::register();
    if args.is_empty() {
        eprintln!("usage: flare-cli <info|gif|png|png-seq|video|svg|svg-anim|html5|sprites|lottie|xfl|convert|save-demo|run|commands|effects|xdts> ...  (use 'demo' as the document for the built-in scene)");
        std::process::exit(2);
    }
    if let Err(e) = run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
