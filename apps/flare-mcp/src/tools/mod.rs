//! The tools an agent can call, grouped by what they touch. Each is a thin layer over the same
//! edit commands, frame operations and renderer that the CLI and desktop app use.

mod draw;
mod history;
mod read;
mod timeline;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::server::FlareServer;

impl FlareServer {
    /// Every tool, from the groups' routers.
    pub(crate) fn router() -> ToolRouter<Self> {
        Self::read_tools() + Self::draw_tools() + Self::timeline_tools() + Self::history_tools()
    }
}
