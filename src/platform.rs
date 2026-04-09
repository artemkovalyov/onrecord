//! Platform-specific window setup.
//!
//! X11/KWin approach:
//! - Overlay: fullscreen_on_monitor() handles geometry. _NET_WM_STATE_ABOVE sent via
//!   idle_add after present() so KWin has mapped the window before the ClientMessage.
//! - Toolbar: _NET_WM_STATE_ABOVE + STICKY set before map via change_property (KWin
//!   reads initial state at map time for non-fullscreen windows).
//!
//! Wayland branch is a stub pending gtk4-layer-shell.

use gtk4::prelude::*;
use gtk4::ApplicationWindow;

// ── public API ────────────────────────────────────────────────────────────────

/// Send _NET_WM_STATE_ABOVE ClientMessage to root. Must be called AFTER present()
/// (e.g. via glib::idle_add_once) so KWin has already mapped the window.
pub fn setup_overlay_pre_map(window: &ApplicationWindow) {
    if !try_set_above_client_message(window) {
        eprintln!("annotations: overlay above setup skipped (not X11 or failed)");
    }
}

/// Set _NET_WM_STATE_ABOVE + STICKY on toolbar BEFORE map via change_property.
/// KWin reads initial _NET_WM_STATE from the property when it first maps the window.
/// Call from connect_realize.
pub fn setup_toolbar_pre_map(window: &ApplicationWindow) {
    if !try_toolbar_pre_map_x11(window) {
        eprintln!("annotations: toolbar pre-map setup skipped (not X11 or failed)");
    }
}

/// Reinforce _NET_WM_STATE_ABOVE + STICKY on toolbar via ClientMessage AFTER map.
/// KWin may ignore change_property on already-mapped windows; ClientMessage is authoritative.
/// Call from connect_map (with AtomicBool guard, same as post_map).
pub fn setup_toolbar_above_post_map(window: &ApplicationWindow) {
    if !try_set_above_client_message(window) {
        eprintln!("annotations: toolbar above post-map skipped (not X11 or failed)");
    }
}

/// Move toolbar to config position after map. Call once from connect_map.
pub fn setup_toolbar_post_map(window: &ApplicationWindow, position: [i32; 2]) {
    if !try_toolbar_post_map_x11(window, position) {
        eprintln!("annotations: toolbar post-map position skipped (not X11 or failed)");
    }
}

/// Read current toolbar position via TranslateCoordinates to root (for save-on-exit).
pub fn get_window_position(window: &ApplicationWindow) -> Option<[i32; 2]> {
    get_window_position_x11(window)
}

// ── X11 helpers ───────────────────────────────────────────────────────────────

fn get_x11_xid(window: &ApplicationWindow) -> Option<u32> {
    let surface = window.surface()?;
    let x11_surface = surface.downcast::<gdk4_x11::X11Surface>().ok()?;
    u32::try_from(x11_surface.xid()).ok()
}

fn x11_connect() -> Option<(x11rb::rust_connection::RustConnection, usize)> {
    x11rb::connect(None)
        .map_err(|e| eprintln!("annotations: x11rb connect failed: {e}"))
        .ok()
}

fn intern_atoms(
    conn: &x11rb::rust_connection::RustConnection,
    names: &[&str],
) -> Option<Vec<u32>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::intern_atom;

    let cookies: Vec<_> = names.iter()
        .map(|n| intern_atom(conn, false, n.as_bytes()).ok())
        .collect();
    let mut atoms = Vec::with_capacity(names.len());
    for cookie in cookies {
        atoms.push(cookie?.reply().ok()?.atom);
    }
    Some(atoms)
}

// ── X11 implementation ────────────────────────────────────────────────────────

/// Send _NET_WM_STATE ClientMessages to root requesting ADD for ABOVE, SKIP_TASKBAR,
/// SKIP_PAGER. Must be called after the window is mapped.
fn try_set_above_client_message(window: &ApplicationWindow) -> bool {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let xid = match get_x11_xid(window) { Some(id) => id, None => return false };
    let (conn, screen_num) = match x11_connect() { Some(c) => c, None => return false };

    let atoms = match intern_atoms(&conn, &[
        "_NET_WM_STATE",
        "_NET_WM_STATE_ABOVE",
        "_NET_WM_STATE_SKIP_TASKBAR",
        "_NET_WM_STATE_SKIP_PAGER",
    ]) { Some(a) => a, None => return false };

    let (wm_state, above, skip_taskbar, skip_pager) = (atoms[0], atoms[1], atoms[2], atoms[3]);
    let root = conn.setup().roots[screen_num].root;
    const ADD: u32 = 1;

    // Two atoms per ClientMessage is allowed by the spec
    for (a1, a2) in [(above, 0u32), (skip_taskbar, skip_pager)] {
        let event = ClientMessageEvent {
            response_type: CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: xid,
            type_: wm_state,
            data: ClientMessageData::from([ADD, a1, a2, 0u32, 0u32]),
        };
        if conn.send_event(
            false, root,
            EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
            event,
        ).is_err() {
            return false;
        }
    }
    conn.flush().is_ok()
}

fn try_toolbar_pre_map_x11(window: &ApplicationWindow) -> bool {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::wrapper::ConnectionExt as _;

    let xid = match get_x11_xid(window) { Some(id) => id, None => return false };
    let (conn, _) = match x11_connect() { Some(c) => c, None => return false };

    let atoms = match intern_atoms(&conn, &[
        "_NET_WM_STATE",
        "_NET_WM_STATE_ABOVE",
        "_NET_WM_STATE_STICKY",
        "_NET_WM_STATE_SKIP_TASKBAR",
        "_NET_WM_STATE_SKIP_PAGER",
    ]) { Some(a) => a, None => return false };

    let (wm_state, above, sticky, skip_tb, skip_pg) = (atoms[0], atoms[1], atoms[2], atoms[3], atoms[4]);

    conn.change_property32(PropMode::REPLACE, xid, wm_state, AtomEnum::ATOM,
        &[above, sticky, skip_tb, skip_pg]).is_ok()
        && conn.flush().is_ok()
}

fn try_toolbar_post_map_x11(window: &ApplicationWindow, position: [i32; 2]) -> bool {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let xid = match get_x11_xid(window) { Some(id) => id, None => return false };
    let (conn, _) = match x11_connect() { Some(c) => c, None => return false };

    conn.configure_window(xid, &ConfigureWindowAux::new()
        .x(position[0]).y(position[1])).is_ok()
        && conn.flush().is_ok()
}

fn get_window_position_x11(window: &ApplicationWindow) -> Option<[i32; 2]> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let xid = get_x11_xid(window)?;
    let (conn, screen_num) = x11_connect()?;
    let root = conn.setup().roots[screen_num].root;
    let reply = conn.translate_coordinates(xid, root, 0, 0).ok()?.reply().ok()?;
    Some([i32::from(reply.dst_x), i32::from(reply.dst_y)])
}
