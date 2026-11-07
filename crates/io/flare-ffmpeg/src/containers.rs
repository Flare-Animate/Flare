//! The containers ffmpeg writes, and how each is made: the muxer, the codecs and the sound.

/// One container ffmpeg writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Container {
    /// What the command line and the MCP tool call the format (`mov`, `mkv`, ...).
    pub id: &'static str,
    /// The file name extension.
    pub extension: &'static str,
    /// What people call it.
    pub label: &'static str,
    /// The ffmpeg muxer, passed as `-f`, so the file name never decides the format.
    pub muxer: &'static str,
    /// The encoder for the pictures.
    pub video_codec: &'static str,
    /// The pixel format the encoder takes; `None` passes the frames as RGBA.
    pub pixel_format: Option<&'static str>,
    /// The video bit rate for a lossy codec; `None` lets the codec choose.
    pub bitrate: Option<&'static str>,
    /// The encoder for the sound; `None` for a format that keeps no sound.
    pub audio_codec: Option<&'static str>,
}

/// Every container [`container`] knows, in the order the command line lists them.
pub const CONTAINERS: [Container; 6] = [
    Container {
        id: "mov",
        extension: "mov",
        label: "QuickTime movie",
        muxer: "mov",
        video_codec: "libx264",
        pixel_format: Some("yuv420p"),
        bitrate: Some("4M"),
        audio_codec: Some("aac"),
    },
    Container {
        id: "avi",
        extension: "avi",
        label: "AVI movie",
        muxer: "avi",
        video_codec: "mpeg4",
        pixel_format: Some("yuv420p"),
        bitrate: Some("4M"),
        audio_codec: Some("pcm_s16le"),
    },
    Container {
        id: "mkv",
        extension: "mkv",
        label: "Matroska movie",
        muxer: "matroska",
        video_codec: "libx264",
        pixel_format: Some("yuv420p"),
        bitrate: Some("4M"),
        audio_codec: Some("aac"),
    },
    Container {
        id: "webm",
        extension: "webm",
        label: "WebM movie",
        muxer: "webm",
        video_codec: "libvpx-vp9",
        pixel_format: Some("yuv420p"),
        bitrate: Some("4M"),
        audio_codec: Some("libopus"),
    },
    Container {
        id: "mpeg",
        extension: "mpg",
        label: "MPEG movie",
        muxer: "mpeg",
        video_codec: "mpeg2video",
        pixel_format: Some("yuv420p"),
        bitrate: Some("4M"),
        audio_codec: Some("mp2"),
    },
    Container {
        id: "flv",
        extension: "flv",
        label: "Flash video",
        muxer: "flv",
        video_codec: "flv",
        pixel_format: Some("yuv420p"),
        bitrate: Some("4M"),
        audio_codec: Some("aac"),
    },
];

/// The container with this `id`, ignoring case.
pub fn container(id: &str) -> Option<&'static Container> {
    CONTAINERS.iter().find(|c| c.id.eq_ignore_ascii_case(id))
}
