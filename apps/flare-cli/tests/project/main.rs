#![allow(clippy::expect_used)]

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

mod editing;
mod formats;
mod layers_and_stage;
mod rendering;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flare-cli"))
        .args(args)
        .output()
        .expect("run cli")
}
fn json(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}
fn path(dir: &Path) -> String {
    dir.join("t.flare").to_string_lossy().into_owned()
}
fn read_png(path: &str) -> (Vec<u8>, u32, u32) {
    let file = std::io::BufReader::new(std::fs::File::open(path).expect("open png"));
    let mut reader = png::Decoder::new(file).read_info().expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    buf.truncate(info.buffer_size());
    (buf, info.width, info.height)
}
fn pixel(image: &(Vec<u8>, u32, u32), x: u32, y: u32) -> [u8; 4] {
    let i = ((y * image.1 + x) * 4) as usize;
    [image.0[i], image.0[i + 1], image.0[i + 2], image.0[i + 3]]
}
