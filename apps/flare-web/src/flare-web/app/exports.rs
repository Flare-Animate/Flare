//! The pictures, movies and documents the page writes for the person: each is made on the page's
//! one thread and offered as a download.

use flare_animate::DialogRequest;

use super::WebApp;

impl WebApp {
    /// Makes the file an export asks for and downloads it; a failure is told to the person.
    pub(super) fn handle_export(&mut self, request: DialogRequest) {
        match request {
            DialogRequest::ExportImage => {
                // The name's ending picks the format, as on the desktop; no ending is a PNG.
                let Some(name) = self.prompt_file_name("frame.png") else {
                    return;
                };
                let name = with_default_ending(name, "png");
                let ending = ending_of(&name);
                let (document, frame) = self.workspace.image_export();
                let backdrop = self.workspace.publish_settings().backdrop();
                let bytes = if flare_runtime::is_still_ending(&ending) {
                    flare_runtime::picture_bytes(&document, frame, &ending, backdrop)
                } else {
                    Err(format!(
                        "A picture is written as .png, .jpg or the other still formats, not .{ending}"
                    ))
                };
                self.deliver(&name, picture_mime(&ending), bytes);
            }
            DialogRequest::ExportMovie => {
                // The browser writes an animated GIF or APNG; MP4, QuickTime and the ffmpeg
                // containers need the desktop app, which has the video encoders.
                let Some(name) = self.prompt_file_name("movie.gif") else {
                    return;
                };
                let name = with_default_ending(name, "gif");
                let ending = ending_of(&name);
                let (document, _) = self.workspace.image_export();
                let backdrop = self.workspace.publish_settings().backdrop();
                let bytes = if matches!(ending.as_str(), "gif" | "apng") {
                    flare_runtime::picture_bytes(&document, 0, &ending, backdrop)
                } else {
                    Err(format!(
                        "The browser writes a movie as .gif or .apng, not .{ending}; MP4, QuickTime and the other video formats need the desktop app."
                    ))
                };
                self.deliver(&name, picture_mime(&ending), bytes);
            }
            DialogRequest::ExportFla => {
                let (document, _) = self.workspace.image_export();
                let bytes = flare_export::export_xfl(&document)
                    .and_then(|output| flare_export::fla_bytes(&output))
                    .map_err(|e| e.to_string());
                self.deliver("project.fla", "application/zip", bytes);
            }
            DialogRequest::ExportSvg => {
                let (document, frame) = self.workspace.image_export();
                let backdrop = self.workspace.publish_settings().backdrop();
                let bytes: Result<Vec<u8>, String> =
                    flare_runtime::svg_export(&document, frame, backdrop)
                        .map(|svg| svg.text.into_bytes());
                self.deliver("frame.svg", "image/svg+xml", bytes);
            }
            DialogRequest::ExportSpriteSheet => {
                // The picture and its atlas are two downloads, named alike.
                let (document, _) = self.workspace.image_export();
                match flare_runtime::sprite_sheet(&document, 0, None, None, "sprites.png") {
                    Ok(sheet) => {
                        self.deliver("sprites.png", "image/png", Ok(sheet.png));
                        self.deliver(
                            "sprites.json",
                            "application/json",
                            Ok(sheet.json.into_bytes()),
                        );
                    }
                    Err(error) => self.workspace.report_message(&error),
                }
            }
            _ => {}
        }
    }
}

/// `name` with `ending` added when it has none, so a name typed without one is written as that
/// format.
fn with_default_ending(name: String, ending: &str) -> String {
    if std::path::Path::new(&name).extension().is_some() {
        name
    } else {
        format!("{name}.{ending}")
    }
}

/// The ending of `name`, lower case; empty when it has none.
fn ending_of(name: &str) -> String {
    std::path::Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// The media type a download of a picture or movie named with `ending` is offered as.
fn picture_mime(ending: &str) -> &'static str {
    match ending {
        "png" | "apng" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
