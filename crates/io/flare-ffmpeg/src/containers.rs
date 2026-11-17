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
    /// Options for the picture encoder, passed after it in order.
    pub video_options: &'static [&'static str],
    /// How the container keeps see-through pictures; `None` when it cannot.
    pub alpha: Option<AlphaVideo>,
}

/// The pictures of a movie with a transparent background: an encoder and a pixel format that
/// carry an alpha channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlphaVideo {
    pub video_codec: &'static str,
    pub pixel_format: &'static str,
    pub bitrate: Option<&'static str>,
    pub video_options: &'static [&'static str],
}

impl Container {
    /// The same container with see-through pictures, or `None` when it cannot hold them.
    pub fn with_alpha(&self) -> Option<Container> {
        let alpha = self.alpha?;
        Some(Container {
            video_codec: alpha.video_codec,
            pixel_format: Some(alpha.pixel_format),
            bitrate: alpha.bitrate,
            video_options: alpha.video_options,
            alpha: None,
            ..*self
        })
    }
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
        video_options: &[],
        // ProRes 4444, the picture editors and compositors take with its alpha.
        alpha: Some(AlphaVideo {
            video_codec: "prores_ks",
            pixel_format: "yuva444p10le",
            bitrate: None,
            video_options: &["-profile:v", "4444"],
        }),
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
        video_options: &[],
        alpha: None,
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
        video_options: &[],
        alpha: None,
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
        video_options: &[],
        // VP9 with its alpha, which browsers play over the page.
        alpha: Some(AlphaVideo {
            video_codec: "libvpx-vp9",
            pixel_format: "yuva420p",
            bitrate: Some("4M"),
            video_options: &[],
        }),
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
        video_options: &[],
        alpha: None,
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
        video_options: &[],
        alpha: None,
    },
];

/// The container with this `id`, ignoring case.
pub fn container(id: &str) -> Option<&'static Container> {
    CONTAINERS.iter().find(|c| c.id.eq_ignore_ascii_case(id))
}
