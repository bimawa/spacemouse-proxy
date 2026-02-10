#![allow(non_upper_case_globals)]

mod connexion_ffi;
mod hid_ffi;
mod objc_ffi;

use connexion_ffi::*;
use hid_ffi::{CFRunLoopGetCurrent, CFRunLoopRun, CFStringRef, kCFRunLoopDefaultMode};
use objc_ffi::*;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

const WS_PORT: u16 = 18944;
const AXIS_SCALE: f64 = 350.0;
const AXIS_CLAMP: f64 = 1.0;
const AXIS_DEADZONE: i16 = 3;
const SMOOTHING: f64 = 0.4;
const FOCUS_CHECK_INTERVAL: u32 = 8;

const TARGET_BUNDLE_IDS: &[&str] = &[
    "com.figma.Desktop",
    "com.google.Chrome",
    "com.apple.Safari",
    "org.mozilla.firefox",
    "company.thebrowser.Browser",
    "com.microsoft.edgemac",
    "com.brave.Browser",
    "com.operasoftware.Opera",
    "com.vivaldi.Vivaldi",
    "org.chromium.Chromium",
];

static BROADCAST_TX: OnceLock<broadcast::Sender<AxisEvent>> = OnceLock::new();
static SMOOTH: Mutex<[f64; 6]> = Mutex::new([0.0; 6]);
static CLIENT_ID: AtomicU16 = AtomicU16::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static WS_CLIENTS: AtomicU32 = AtomicU32::new(0);
static TICK_COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Serialize)]
struct AxisEvent {
    axes: [f64; 6],
    buttons: u32,
}

fn deadzone(v: i16) -> i16 {
    if v.abs() <= AXIS_DEADZONE { 0 } else { v }
}

fn dispatch_axes(raw: [i16; 6], buttons: u32) {
    let target: [f64; 6] = std::array::from_fn(|i| {
        (deadzone(raw[i]) as f64 / AXIS_SCALE).clamp(-AXIS_CLAMP, AXIS_CLAMP)
    });

    let mut smooth = SMOOTH.lock().unwrap();
    for i in 0..6 {
        smooth[i] = smooth[i] * (1.0 - SMOOTHING) + target[i] * SMOOTHING;
        if smooth[i].abs() < 0.003 {
            smooth[i] = 0.0;
        }
    }
    let axes = *smooth;
    drop(smooth);

    let event = AxisEvent { axes, buttons };

    if let Some(tx) = BROADCAST_TX.get() {
        let _ = tx.send(event);
    }
}

extern "C" fn connexion_message(
    _product_id: u32,
    message_type: u32,
    message_argument: *mut c_void,
) {
    if message_type == kConnexionMsgDeviceState && !message_argument.is_null() {
        let state = unsafe { &*(message_argument as *const ConnexionDeviceState) };

        match state.command {
            kConnexionCmdHandleAxis | kConnexionCmdHandleRawData => {
                dispatch_axes(state.axis, state.buttons);
            }
            kConnexionCmdHandleButtons => {
                dispatch_axes([0; 6], state.buttons);
            }
            _ => {}
        }
    }
}

extern "C" fn connexion_added(product_id: u32) {
    eprintln!("[3dx] Device added: product_id={product_id:#06x}");
}

extern "C" fn connexion_removed(product_id: u32) {
    eprintln!("[3dx] Device removed: product_id={product_id:#06x}");
}

unsafe fn is_target_app_frontmost() -> bool {
    let ws_class = unsafe { objc_getClass(b"NSWorkspace\0".as_ptr()) };
    let workspace = unsafe { msg_send_0(ws_class as id, b"sharedWorkspace\0") };
    let front_app = unsafe { msg_send_0(workspace, b"frontmostApplication\0") };
    if front_app.is_null() {
        return false;
    }
    let bundle_nsstring = unsafe { msg_send_0(front_app, b"bundleIdentifier\0") };
    if bundle_nsstring.is_null() {
        return false;
    }
    let utf8 = unsafe { msg_send_0(bundle_nsstring, b"UTF8String\0") } as *const i8;
    if utf8.is_null() {
        return false;
    }
    let bid = unsafe { std::ffi::CStr::from_ptr(utf8) }.to_bytes();
    TARGET_BUNDLE_IDS.iter().any(|t| bid.starts_with(t.as_bytes()))
}

fn activate_client() {
    let err = unsafe {
        SetConnexionHandlers(connexion_message, connexion_added, connexion_removed, false)
    };
    if err != 0 {
        eprintln!("[focus] SetConnexionHandlers failed: {err}");
        return;
    }

    let client_name = b"\x10SpaceMouse Proxy";
    let cid = unsafe {
        RegisterConnexionClient(
            kConnexionClientManual,
            client_name.as_ptr(),
            kConnexionClientModeTakeOver,
            kConnexionMaskAll,
        )
    };
    if cid == 0 {
        unsafe { CleanupConnexionHandlers() };
        return;
    }

    unsafe { SetConnexionClientButtonMask(cid, kConnexionMaskAllButtons) };
    CLIENT_ID.store(cid, Ordering::Relaxed);

    let mut result: i32 = 0;
    unsafe { ConnexionClientControl(cid, kConnexionCtlActivateClient, 0, &mut result) };

    ACTIVE.store(true, Ordering::Relaxed);
    eprintln!("[focus] activated — capturing SpaceMouse (client_id={cid})");
}

fn deactivate_client() {
    ACTIVE.store(false, Ordering::Relaxed);

    let cid = CLIENT_ID.swap(0, Ordering::Relaxed);
    if cid != 0 {
        unsafe {
            UnregisterConnexionClient(cid);
            CleanupConnexionHandlers();
        };
    }

    {
        let mut smooth = SMOOTH.lock().unwrap();
        *smooth = [0.0; 6];
    }
    if let Some(tx) = BROADCAST_TX.get() {
        let _ = tx.send(AxisEvent { axes: [0.0; 6], buttons: 0 });
    }

    eprintln!("[focus] deactivated — yielding to other apps");
}

async fn run_ws_server(tx: broadcast::Sender<AxisEvent>) {
    let listener = TcpListener::bind(format!("127.0.0.1:{WS_PORT}"))
        .await
        .expect("Failed to bind WebSocket port");

    eprintln!("WebSocket server listening on ws://127.0.0.1:{WS_PORT}");

    while let Ok((stream, addr)) = listener.accept().await {
        let rx = tx.subscribe();
        tokio::spawn(handle_ws_client(stream, rx, addr));
    }
}

async fn handle_ws_client(
    stream: tokio::net::TcpStream,
    mut rx: broadcast::Receiver<AxisEvent>,
    addr: std::net::SocketAddr,
) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake failed from {addr}: {e}");
            return;
        }
    };

    eprintln!("[ws] Client connected: {addr}");
    WS_CLIENTS.fetch_add(1, Ordering::Relaxed);
    let (mut write, mut read) = ws_stream.split();
    let mut interval = tokio::time::interval(Duration::from_millis(16));
    let mut latest: Option<AxisEvent> = None;

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(ev) => { latest = Some(ev); }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
            _ = interval.tick() => {
                if let Some(ev) = latest.take() {
                    let json = serde_json::to_string(&ev).unwrap();
                    if write.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(_)) => {}
                    _ => break,
                }
            }
        }
    }

    WS_CLIENTS.fetch_sub(1, Ordering::Relaxed);
    eprintln!("[ws] Client disconnected: {addr}");
}

extern "C" fn runloop_timer_callback(
    _timer: *mut c_void,
    _info: *mut c_void,
) {
    let tick = TICK_COUNTER.fetch_add(1, Ordering::Relaxed);

    if tick % FOCUS_CHECK_INTERVAL == 0 {
        let has_clients = WS_CLIENTS.load(Ordering::Relaxed) > 0;
        let target_front = unsafe { is_target_app_frontmost() };
        let should_be_active = has_clients && target_front;
        let currently_active = ACTIVE.load(Ordering::Relaxed);

        if should_be_active && !currently_active {
            activate_client();
        } else if !should_be_active && currently_active {
            deactivate_client();
        }
    }

    let cid = CLIENT_ID.load(Ordering::Relaxed);
    if cid != 0 && ACTIVE.load(Ordering::Relaxed) {
        let mut result: i32 = 0;
        unsafe {
            ConnexionClientControl(cid, kConnexionCtlActivateClient, 0, &mut result);
        };
    }
}

unsafe extern "C" {
    fn CFRunLoopTimerCreate(
        allocator: *const c_void,
        fire_date: f64,
        interval: f64,
        flags: u64,
        order: i64,
        callback: extern "C" fn(*mut c_void, *mut c_void),
        context: *mut c_void,
    ) -> *mut c_void;
    fn CFRunLoopAddTimer(
        rl: *const c_void,
        timer: *mut c_void,
        mode: CFStringRef,
    );
    fn CFAbsoluteTimeGetCurrent() -> f64;
}

unsafe extern "C" fn shutdown_handler(_sig: libc::c_int) {
    let cid = CLIENT_ID.load(Ordering::Relaxed);
    if cid != 0 {
        unsafe {
            UnregisterConnexionClient(cid);
            CleanupConnexionHandlers();
        }
    }
    eprintln!("\n[shutdown] Unregistered client, cleaned up handlers.");
    std::process::exit(0);
}

fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGINT, shutdown_handler as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, shutdown_handler as *const () as libc::sighandler_t);
    }
}

fn main() {
    eprintln!("spacemouse-proxy v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("WebSocket: ws://127.0.0.1:{WS_PORT}");
    eprintln!("Waiting for Figma plugin to connect...\n");

    install_signal_handlers();

    let (tx, _) = broadcast::channel::<AxisEvent>(64);
    BROADCAST_TX.set(tx.clone()).unwrap();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(run_ws_server(tx));
    });

    unsafe { init_nsapp_accessory() };

    unsafe {
        let run_loop = CFRunLoopGetCurrent();
        let timer = CFRunLoopTimerCreate(
            std::ptr::null(),
            CFAbsoluteTimeGetCurrent() + 0.1,
            0.016,
            0,
            0,
            runloop_timer_callback,
            std::ptr::null_mut(),
        );
        CFRunLoopAddTimer(run_loop, timer, kCFRunLoopDefaultMode);
        CFRunLoopRun();
    }
}
