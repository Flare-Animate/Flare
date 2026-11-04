//! Parses BVH text one line at a time, so every error carries its line number.

use super::{
    Bvh, BvhError, Joint, MAX_BYTES, MAX_DEPTH, MAX_FRAMES, MAX_JOINTS, MAX_MOTION_VALUES, Motion,
};

/// What is open at the current nesting level: a joint, or the End Site inside one.
enum Open {
    Joint(usize),
    EndSite,
}

pub fn parse(text: &str) -> Result<Bvh, BvhError> {
    if text.len() > MAX_BYTES {
        return Err(BvhError::TooLarge {
            size: text.len(),
            limit: MAX_BYTES,
        });
    }
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty());
    let (mut last, header) = next_line(&mut lines, 0)?;
    if header != "HIERARCHY" {
        return Err(syntax(last, "expected HIERARCHY"));
    }

    let mut bvh = Bvh::default();
    let mut open: Vec<Open> = Vec::new();
    loop {
        let (line, content) = next_line(&mut lines, last)?;
        last = line;
        let tokens: Vec<&str> = content.split_whitespace().collect();
        let current = joint_on(&open);
        let in_end_site = matches!(open.last(), Some(Open::EndSite));
        match tokens.as_slice() {
            ["MOTION"] => {
                if !open.is_empty() {
                    return Err(syntax(line, "a joint is not closed before MOTION"));
                }
                break;
            }
            [keyword @ ("ROOT" | "JOINT"), name] => {
                if bvh.joints.len() >= MAX_JOINTS {
                    return Err(BvhError::Limit {
                        what: "joints",
                        limit: MAX_JOINTS,
                    });
                }
                let parent = match (*keyword, current) {
                    ("ROOT", _) if !bvh.joints.is_empty() => {
                        return Err(syntax(line, "a BVH file has one ROOT"));
                    }
                    ("ROOT", _) => None,
                    (_, Some(parent)) => Some(parent),
                    (_, None) => return Err(syntax(line, "JOINT outside ROOT")),
                };
                if open.len() >= MAX_DEPTH {
                    return Err(BvhError::Limit {
                        what: "joint depth",
                        limit: MAX_DEPTH,
                    });
                }
                expect_brace(&mut lines, &mut last)?;
                bvh.joints.push(Joint {
                    name: (*name).to_owned(),
                    parent,
                    offset: [0.0; 3],
                    channels: Vec::new(),
                    end_site: None,
                });
                open.push(Open::Joint(bvh.joints.len() - 1));
            }
            ["OFFSET", x, y, z] => {
                let offset = [number(x, line)?, number(y, line)?, number(z, line)?];
                let joint = current.ok_or_else(|| syntax(line, "OFFSET outside a joint"))?;
                let target = joint_mut(&mut bvh, joint, line)?;
                if in_end_site {
                    target.end_site = Some(offset);
                } else {
                    target.offset = offset;
                }
            }
            ["CHANNELS", count, names @ ..] => {
                let count: usize = count
                    .parse()
                    .map_err(|_| syntax(line, "CHANNELS needs a count"))?;
                if names.len() != count {
                    return Err(syntax(line, "CHANNELS count does not match its names"));
                }
                let channels = names
                    .iter()
                    .map(|n| {
                        Motion::from_name(n)
                            .ok_or_else(|| syntax(line, format!("unknown channel {n}")))
                    })
                    .collect::<Result<Vec<Motion>, BvhError>>()?;
                let joint = current.ok_or_else(|| syntax(line, "CHANNELS outside a joint"))?;
                if in_end_site {
                    return Err(syntax(line, "CHANNELS inside an End Site"));
                }
                joint_mut(&mut bvh, joint, line)?.channels = channels;
            }
            ["End", "Site"] => {
                if current.is_none() {
                    return Err(syntax(line, "End Site outside a joint"));
                }
                if open.len() >= MAX_DEPTH {
                    return Err(BvhError::Limit {
                        what: "joint depth",
                        limit: MAX_DEPTH,
                    });
                }
                expect_brace(&mut lines, &mut last)?;
                open.push(Open::EndSite);
            }
            ["}"] => {
                if open.pop().is_none() {
                    return Err(syntax(line, "} with nothing open"));
                }
            }
            _ => return Err(syntax(line, format!("unexpected line: {content}"))),
        }
    }

    let (line, frames_line) = next_line(&mut lines, last)?;
    let frames: Vec<&str> = frames_line.split_whitespace().collect();
    let frame_count: usize = match frames.as_slice() {
        ["Frames:", n] => n
            .parse()
            .map_err(|_| syntax(line, "Frames: needs a count"))?,
        _ => return Err(syntax(line, "expected Frames: <count>")),
    };
    let (line, time_line) = next_line(&mut lines, line)?;
    let times: Vec<&str> = time_line.split_whitespace().collect();
    let frame_time = match times.as_slice() {
        ["Frame", "Time:", t] => number(t, line)?,
        _ => return Err(syntax(line, "expected Frame Time: <seconds>")),
    };

    let width = bvh.channel_count();
    if frame_count > MAX_FRAMES {
        return Err(BvhError::Limit {
            what: "frames",
            limit: MAX_FRAMES,
        });
    }
    let total = frame_count
        .checked_mul(width)
        .filter(|n| *n <= MAX_MOTION_VALUES)
        .ok_or(BvhError::Limit {
            what: "motion values",
            limit: MAX_MOTION_VALUES,
        })?;
    let mut motion = Vec::with_capacity(total);
    let mut line = line;
    for _ in 0..frame_count {
        let (row_line, row) = next_line(&mut lines, line)?;
        line = row_line;
        let before = motion.len();
        for token in row.split_whitespace() {
            motion.push(number(token, row_line)?);
        }
        let found = motion.len() - before;
        if found != width {
            return Err(syntax(
                row_line,
                format!("{found} values where {width} are needed"),
            ));
        }
    }
    if let Some((extra, _)) = lines.next() {
        return Err(syntax(extra, "more motion rows than Frames: says"));
    }

    bvh.frame_time = frame_time;
    bvh.frame_count = frame_count;
    bvh.motion = motion;
    bvh.validate()?;
    Ok(bvh)
}

fn next_line<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
    last: usize,
) -> Result<(usize, &'a str), BvhError> {
    lines
        .next()
        .ok_or_else(|| syntax(last, "unexpected end of file"))
}

fn expect_brace<'a>(
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
    last: &mut usize,
) -> Result<(), BvhError> {
    let (line, content) = next_line(lines, *last)?;
    *last = line;
    if content == "{" {
        Ok(())
    } else {
        Err(syntax(line, "expected {"))
    }
}

/// The nearest joint that is open, skipping any End Site inside it.
fn joint_on(open: &[Open]) -> Option<usize> {
    open.iter().rev().find_map(|o| match o {
        Open::Joint(i) => Some(*i),
        Open::EndSite => None,
    })
}

fn joint_mut(bvh: &mut Bvh, index: usize, line: usize) -> Result<&mut Joint, BvhError> {
    bvh.joints
        .get_mut(index)
        .ok_or_else(|| syntax(line, "no such joint"))
}

fn number(token: &str, line: usize) -> Result<f64, BvhError> {
    let value: f64 = token
        .parse()
        .map_err(|_| syntax(line, format!("{token} is not a number")))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(syntax(line, "values must be finite"))
    }
}

fn syntax(line: usize, message: impl Into<String>) -> BvhError {
    BvhError::Syntax {
        line,
        message: message.into(),
    }
}
