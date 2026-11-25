//! The tools an agent can call, grouped by what they touch. Each is a thin layer over the same
//! edit commands, frame operations and renderer that the CLI and desktop app use.

mod bend_edge;
mod bones;
mod brushes;
mod draw;
mod effects;
mod export;
mod frames;
mod game;
mod history;
mod import_into;
mod layer_ops;
mod magnet;
mod mannequin;
mod multi_frame;
mod pixels;
mod polygon;
mod read;
mod rigs;
mod scripts;
mod shaders;
mod shortcuts;
mod sound;
mod stop_motion;
mod stopmotion;
#[cfg(test)]
mod stopmotion_tests;
mod symbols;
mod text;
mod timeline;
mod tween;
mod video;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::server::FlareServer;

impl FlareServer {
    /// Every tool, from the groups' routers.
    pub(crate) fn router() -> ToolRouter<Self> {
        Self::read_tools()
            + Self::export_tools()
            + Self::draw_tools()
            + Self::polygon_tools()
            + Self::frame_tools()
            + Self::text_tools()
            + Self::timeline_tools()
            + Self::layer_op_tools()
            + Self::effect_tools()
            + Self::multi_frame_tools()
            + Self::history_tools()
            + Self::bone_tools()
            + Self::brush_tools()
            + Self::stopmotion_tools()
            + Self::sound_tools()
            + Self::import_into_tools()
            + Self::tween_tools()
            + Self::shortcut_tools()
            + Self::shader_tools()
            + Self::symbol_tools()
            + Self::game_tools()
            + Self::pixel_tools()
            + Self::bend_edge_tools()
            + Self::magnet_tools()
            + Self::project_rig_tools()
            + Self::stop_motion_tools()
            + Self::mannequin_tools()
            + Self::video_tools()
            + Self::script_tools()
    }
}
