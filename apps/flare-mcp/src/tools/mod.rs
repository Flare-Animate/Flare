//! The tools an agent can call, grouped by what they touch. Each is a thin layer over the same
//! edit commands, frame operations and renderer that the CLI and desktop app use.

mod draw;
mod export;
mod history;
mod read;
mod stopmotion;
#[cfg(test)]
mod stopmotion_tests;
mod symbols;
mod text;
mod timeline;
mod tween;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::server::FlareServer;

impl FlareServer {
    /// Every tool, from the groups' routers.
    pub(crate) fn router() -> ToolRouter<Self> {
        Self::read_tools()
            + Self::export_tools()
            + Self::draw_tools()
            + Self::text_tools()
            + Self::timeline_tools()
            + Self::history_tools()
            + Self::stopmotion_tools()
            + Self::tween_tools()
            + Self::symbol_tools()
    }
}
