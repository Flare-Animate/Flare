//! Pen pressure on Linux. winit gives a tablet pen to the program as a mouse, with no pressure,
//! so the pen is read from the X server directly: XInput2 events for the pen devices are selected
//! on the window by a second connection, and the pressure goes into a [`PenFeed`] that the stage
//! reads. Under Wayland this works through XWayland, which the compositor feeds from its own
//! tablet protocol.

use std::thread;
use std::time::Instant;

use flare_input::PenFeed;
use thiserror::Error;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{self, EventMask, XIEventMask};
use x11rb::protocol::xproto::{ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

use crate::tablet_events::{Pen, Tracker, pens};

#[derive(Debug, Error)]
pub enum TabletError {
    /// There is no X server to ask, or it cannot do XInput2 (version 2.0 is needed).
    #[error("pen pressure is not available: {0}")]
    Unavailable(String),
}

fn unavailable(error: impl std::fmt::Display) -> TabletError {
    TabletError::Unavailable(error.to_string())
}

/// XInput2 version 2.0 is all that is asked of the server.
fn connect() -> Result<(RustConnection, usize), TabletError> {
    let (conn, screen) = RustConnection::connect(None).map_err(unavailable)?;
    let version = xinput::xi_query_version(&conn, 2, 0)
        .map_err(unavailable)?
        .reply()
        .map_err(unavailable)?;
    if version.major_version < 2 {
        return Err(TabletError::Unavailable("the server has no XInput2".into()));
    }
    Ok((conn, screen))
}

/// The pen devices the server has right now.
fn find_pens(conn: &RustConnection) -> Result<Vec<Pen>, TabletError> {
    let label = conn
        .intern_atom(true, b"Abs Pressure")
        .map_err(unavailable)?
        .reply()
        .map_err(unavailable)?
        .atom;
    // 0 is XIAllDevices. A server that has never heard of the label has no pressure axis.
    if label == x11rb::NONE {
        return Ok(Vec::new());
    }
    let devices = xinput::xi_query_device(conn, 0u16)
        .map_err(unavailable)?
        .reply()
        .map_err(unavailable)?;
    Ok(pens(&devices.infos, label))
}

/// Whether the X server has a pen to read pressure from. The window system is chosen by this,
/// before a window exists.
pub fn has_pressure_device() -> bool {
    connect()
        .and_then(|(conn, _)| find_pens(&conn))
        .is_ok_and(|found| !found.is_empty())
}

fn select(conn: &RustConnection, window: Window, found: &[Pen]) -> Result<(), TabletError> {
    if found.is_empty() {
        return Ok(());
    }
    let events = XIEventMask::MOTION | XIEventMask::BUTTON_PRESS | XIEventMask::BUTTON_RELEASE;
    let masks: Vec<EventMask> = found
        .iter()
        .map(|pen| EventMask {
            deviceid: pen.device,
            mask: vec![events],
        })
        .collect();
    xinput::xi_select_events(conn, window, &masks).map_err(unavailable)?;
    conn.flush().map_err(unavailable)
}

/// Starts feeding `feed` with the pen's pressure over the X window `window`, from a thread of
/// its own that ends with the process. Pens plugged in later are picked up.
pub fn watch(window: u32, feed: PenFeed) -> Result<(), TabletError> {
    let (conn, screen) = connect()?;
    let root = conn.setup().roots[screen].root;
    let mut found = find_pens(&conn)?;
    select(&conn, window, &found)?;
    // Devices appearing or going are announced on the root window.
    xinput::xi_select_events(
        &conn,
        root,
        &[EventMask {
            deviceid: 0,
            mask: vec![XIEventMask::HIERARCHY],
        }],
    )
    .map_err(unavailable)?;
    conn.flush().map_err(unavailable)?;
    thread::Builder::new()
        .name("pen-pressure".into())
        .spawn(move || {
            let mut tracker = Tracker::default();
            // The connection failing is the server going away, which ends the window too.
            while let Ok(event) = conn.wait_for_event() {
                let now = Instant::now();
                match event {
                    Event::XinputMotion(e) => tracker.moved(&e, &found, &feed, now),
                    Event::XinputButtonPress(e) => tracker.button(&e, true, &found, &feed, now),
                    Event::XinputButtonRelease(e) => tracker.button(&e, false, &found, &feed, now),
                    Event::XinputHierarchy(_) => {
                        if let Ok(now_found) = find_pens(&conn)
                            && select(&conn, window, &now_found).is_ok()
                        {
                            found = now_found;
                        }
                    }
                    _ => {}
                }
            }
        })
        .map_err(unavailable)?;
    Ok(())
}
