//! Finding the pose in a picture file, as the command line and the MCP server are given one:
//! the file is read (no larger than a picture may be), kept no larger than the detector needs,
//! and looked in.

use flare_mannequin::Marks;

/// The longest side a picture is looked at, in pixels: the detector's looks are 192 pixels
/// across, so more only costs time.
pub const LOOK_SIDE: u32 = 1024;

/// The marks of the person in the picture file `name` (its kind told by its ending), in the
/// picture's own pixels; an error says the file could not be read or no one was found in it.
pub fn find_pose_in_file(name: &str, bytes: &[u8]) -> Result<Marks, String> {
    let still = flare_import::read_still(name, bytes, LOOK_SIDE).map_err(|e| e.to_string())?;
    let pixels = crate::Pixels {
        rgba: &still.rgba,
        size: still.kept,
    };
    let picture = [f64::from(still.width), f64::from(still.height)];
    crate::find_pose(pixels, picture)
        .map_err(|e| e.to_string())?
        .map(|found| found.marks)
        .ok_or_else(|| format!("no person was found in {name}"))
}
