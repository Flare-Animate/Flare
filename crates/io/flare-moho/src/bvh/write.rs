//! Writes a [`Bvh`] as BVH text. Reading the text back gives the same value.

use std::fmt::Write;

use super::{Bvh, BvhError};

pub fn to_text(bvh: &Bvh) -> Result<String, BvhError> {
    bvh.validate()?;
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); bvh.joints.len()];
    let mut root = 0;
    for (index, joint) in bvh.joints.iter().enumerate() {
        match joint.parent {
            Some(parent) => {
                if let Some(list) = children.get_mut(parent) {
                    list.push(index);
                }
            }
            None => root = index,
        }
    }

    let mut out = String::from("HIERARCHY\n");
    write_joint(bvh, &children, root, 0, &mut out);
    out.push_str("MOTION\n");
    let _ = writeln!(out, "Frames: {}", bvh.frame_count);
    let _ = writeln!(out, "Frame Time: {}", bvh.frame_time);
    let width = bvh.channel_count();
    if width > 0 {
        for row in bvh.motion.chunks_exact(width) {
            let values: Vec<String> = row.iter().map(f64::to_string).collect();
            out.push_str(&values.join(" "));
            out.push('\n');
        }
    }
    Ok(out)
}

fn write_joint(bvh: &Bvh, children: &[Vec<usize>], index: usize, depth: usize, out: &mut String) {
    let Some(joint) = bvh.joints.get(index) else {
        return;
    };
    let indent = "\t".repeat(depth);
    let inner = format!("{indent}\t");
    let keyword = if joint.parent.is_none() {
        "ROOT"
    } else {
        "JOINT"
    };
    let _ = writeln!(out, "{indent}{keyword} {}", joint.name);
    let _ = writeln!(out, "{indent}{{");
    let [x, y, z] = joint.offset;
    let _ = writeln!(out, "{inner}OFFSET {x} {y} {z}");
    let names: Vec<&str> = joint.channels.iter().map(|c| c.name()).collect();
    let _ = writeln!(out, "{inner}CHANNELS {} {}", names.len(), names.join(" "));
    if let Some(list) = children.get(index) {
        for &child in list {
            write_joint(bvh, children, child, depth + 1, out);
        }
    }
    if let Some([x, y, z]) = joint.end_site {
        let _ = writeln!(out, "{inner}End Site");
        let _ = writeln!(out, "{inner}{{");
        let _ = writeln!(out, "{inner}\tOFFSET {x} {y} {z}");
        let _ = writeln!(out, "{inner}}}");
    }
    let _ = writeln!(out, "{indent}}}");
}
