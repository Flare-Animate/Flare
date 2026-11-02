//! Pen pressure on Linux. winit gives a tablet pen to the program as a mouse, with no pressure,
//! so the pen is read from the X server directly: XInput2 raw events, selected on the root
//! window by a second connection, go into a [`PenFeed`] that the stage reads. Under Wayland this
//! works through XWayland, which the compositor feeds from its own tablet protocol.

use std::thread;

use flare_input::PenFeed;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{self, EventMask, XIEventMask};
use x11rb::protocol::xproto::{ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

use crate::TabletError;
use crate::x11_events::{State, Update, devices};

fn unavailable(error: impl std::fmt::Display) -> TabletError {
    TabletError::Unavailable(error.to_string())
}

/// `XIAllDevices`.
const ALL_DEVICES: u16 = 0;

fn connect() -> Result<(RustConnection, Window), TabletError> {
    let (conn, screen) = RustConnection::connect(None).map_err(unavailable)?;
    let root = conn
        .setup()
        .roots
        .get(screen)
        .ok_or_else(|| unavailable("no screen"))?
        .root;
    let version = xinput::xi_query_version(&conn, 2, 2)
        .map_err(unavailable)?
        .reply()
        .map_err(unavailable)?;
    if (version.major_version, version.minor_version) < (2, 1) {
        return Err(TabletError::Unavailable(format!(
            "XInput {}.{} has no raw events",
            version.major_version, version.minor_version
        )));
    }
    Ok((conn, root))
}

/// The pointer devices the server has right now.
fn query(conn: &RustConnection) -> Result<State, TabletError> {
    // Asked each time: the server makes the atom when a driver first names a pressure axis, so
    // a pen plugged in later may be the first to have it. `true` is "only if it exists".
    let label = conn
        .intern_atom(true, b"Abs Pressure")
        .map_err(unavailable)?
        .reply()
        .map_err(unavailable)?
        .atom;
    let mut state = State::default();
    if label == x11rb::NONE {
        return Ok(state);
    }
    let infos = xinput::xi_query_device(conn, ALL_DEVICES)
        .map_err(unavailable)?
        .reply()
        .map_err(unavailable)?
        .infos;
    state.set_devices(devices(&infos, label));
    Ok(state)
}

/// Asks for device changes always, and for every pointer's raw motion and buttons only while a
/// pen exists: with none, a person's mouse would wake this thread for nothing.
fn select(conn: &RustConnection, root: Window, raw: bool) -> Result<(), TabletError> {
    let mut mask = XIEventMask::HIERARCHY | XIEventMask::DEVICE_CHANGED;
    if raw {
        mask = mask
            | XIEventMask::RAW_MOTION
            | XIEventMask::RAW_BUTTON_PRESS
            | XIEventMask::RAW_BUTTON_RELEASE;
    }
    xinput::xi_select_events(
        conn,
        root,
        &[EventMask {
            deviceid: ALL_DEVICES,
            mask: vec![mask],
        }],
    )
    .map_err(unavailable)?;
    conn.flush().map_err(unavailable)
}

fn apply(feed: &PenFeed, update: Update) {
    match update {
        Update::Keep => {}
        Update::Pen(pressure) => feed.set(Some(pressure)),
        Update::Mouse => feed.set(None),
    }
}

/// Starts feeding `feed` with the pen's pressure, from a thread of its own that ends with the
/// process, and returns the names of the pens found now. Pens plugged in later are picked up.
/// Call it only when the window is an X11 one: on native Wayland `$DISPLAY` is XWayland, which
/// sees none of this window's input.
pub fn watch(feed: PenFeed) -> Result<Vec<String>, TabletError> {
    let (conn, root) = connect()?;
    let mut state = query(&conn)?;
    select(&conn, root, state.has_pen())?;
    let names = state.pens();
    thread::Builder::new()
        .name("pen-pressure".into())
        .spawn(move || {
            // The connection failing is the server going away, which ends the window too.
            while let Ok(event) = conn.wait_for_event() {
                match event {
                    Event::XinputRawMotion(e)
                    | Event::XinputRawButtonPress(e)
                    | Event::XinputRawButtonRelease(e) => {
                        apply(
                            &feed,
                            state.handle(e.deviceid, &e.valuator_mask, &e.axisvalues),
                        );
                    }
                    Event::XinputHierarchy(_) | Event::XinputDeviceChanged(_) => {
                        // A pen that was unplugged mid-stroke is not seen lifting.
                        feed.set(None);
                        if let Ok(now) = query(&conn)
                            && select(&conn, root, now.has_pen()).is_ok()
                        {
                            state = now;
                        }
                    }
                    _ => {}
                }
            }
        })
        .map_err(unavailable)?;
    Ok(names)
}
