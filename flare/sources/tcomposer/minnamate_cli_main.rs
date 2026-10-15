use std::path::PathBuf;
use std::process::ExitCode;

use minnamate_core::{Result, render::render, swf_export, xfl};

const USAGE: &str = "usage: minnamate-cli render <file.fla | xfl-folder> [--frame N] [--scale S] [-o out.png]
       minnamate-cli swf <file.fla | xfl-folder> [-o out.swf]
  --frame N   1-based, as in Animate's timeline (default 1)";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<()> {
    let mut args = args.into_iter();
    let command = args.next().unwrap_or_default();
    let (mut input, mut frame, mut scale, mut out) = (None, 1u32, 1.0f32, None);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "-f" | "--frame" => frame = value()?.parse()?,
            "-s" | "--scale" => scale = value()?.parse()?,
            "-o" | "--out" => out = Some(PathBuf::from(value()?)),
            _ if input.is_none() && !arg.starts_with('-') => input = Some(PathBuf::from(&arg)),
            _ => return Err(USAGE.into()),
        }
    }
    let input = input.ok_or(USAGE)?;
    let (out, skipped) = match command.as_str() {
        "render" => {
            let doc = xfl::load(&input)?;
            let (pixmap, skipped) = render(&doc, frame.checked_sub(1).ok_or("frames start at 1")?, scale)?;
            let out = out.unwrap_or_else(|| "frame.png".into());
            pixmap.save_png(&out)?;
            (out, skipped)
        }
        "swf" => {
            let (bytes, skipped) = swf_export::export(&xfl::load(&input)?)?;
            let out = out.unwrap_or_else(|| input.with_extension("swf"));
            std::fs::write(&out, bytes)?;
            (out, skipped)
        }
        _ => return Err(USAGE.into()),
    };
    for s in skipped {
        eprintln!("warning: skipped {s}");
    }
    println!("wrote {}", out.display());
    Ok(())
}
