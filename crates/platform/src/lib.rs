use codeswitch_engine::{Action, Input, Layout, Snapshot};
use std::ffi::{c_char, c_void, CStr};
use std::sync::{
    mpsc::{sync_channel, Receiver, SyncSender},
    OnceLock,
};

#[repr(C)]
struct NativeEvent {
    revision: u64,
    kind: u32,
}

#[repr(C)]
struct RawSnapshot {
    handle: *mut c_void,
    field: u64,
    caret: i64,
    revision: u64,
    layout: u32,
    suffix: [u16; 192],
    length: u32,
    app: [c_char; 512],
}

unsafe extern "C" {
    fn cs_start(
        callback: extern "C" fn(*const NativeEvent, *mut c_void),
        context: *mut c_void,
    ) -> i32;
    fn cs_stop();
    fn cs_permissions(request: i32) -> u32;
    fn cs_layouts() -> u32;
    fn cs_revision() -> u64;
    fn cs_snapshot(output: *mut RawSnapshot) -> i32;
    fn cs_release(handle: *mut c_void);
    fn cs_replace(
        before: *const RawSnapshot,
        expected: *const u16,
        expected_len: u32,
        replacement: *const u16,
        replacement_len: u32,
        layout: u32,
    ) -> i32;
    fn cs_finish(replay: i32);
    fn cs_shortcut(modifiers: u32, key: u32);
    fn cs_capture(enabled: i32);
}

pub enum EventKind {
    Input(Input),
    Complete,
}
pub struct Event {
    pub revision: u64,
    pub kind: EventKind,
}
static SENDER: OnceLock<SyncSender<Event>> = OnceLock::new();

extern "C" fn callback(event: *const NativeEvent, _: *mut c_void) {
    if event.is_null() {
        return;
    }
    // The C callback supplies valid storage for the duration of this call. It never
    // blocks on the worker: an overflow is detected through the revision gap.
    let event = unsafe { &*event };
    let kind = match event.kind {
        // No key text crosses this callback. The worker fills it from a verified
        // accessibility snapshot after the target app processes the physical key.
        1 => EventKind::Input(Input::Text(String::new())),
        2 => EventKind::Input(Input::Backspace),
        4 => EventKind::Input(Input::Undo),
        5 => EventKind::Complete,
        _ => EventKind::Input(Input::Reset),
    };
    if let Some(sender) = SENDER.get() {
        let _ = sender.try_send(Event {
            revision: event.revision,
            kind,
        });
    }
}

pub fn start() -> Result<Receiver<Event>, String> {
    let (sender, receiver) = sync_channel(128);
    SENDER
        .set(sender)
        .map_err(|_| "Keyboard worker already started")?;
    if unsafe { cs_start(callback, std::ptr::null_mut()) } == 0 {
        return Err("Native keyboard integration failed to start".into());
    }
    Ok(receiver)
}

pub fn stop() {
    unsafe { cs_stop() }
}
pub fn permission(request: bool) -> bool {
    unsafe { cs_permissions(i32::from(request)) & 1 != 0 }
}
pub fn hook_active() -> bool {
    unsafe { cs_permissions(0) & 2 != 0 }
}
pub fn layouts() -> (bool, bool) {
    let flags = unsafe { cs_layouts() };
    (flags & 1 != 0, flags & 2 != 0)
}
pub fn revision() -> u64 {
    unsafe { cs_revision() }
}
pub fn finish(replay: bool) {
    unsafe { cs_finish(i32::from(replay)) }
}
pub fn capture(enabled: bool) {
    unsafe { cs_capture(i32::from(enabled)) }
}

pub fn shortcut(value: &str) -> Result<(), String> {
    let mut mods = 0;
    let mut key = 0;
    for part in value.split('+') {
        match part {
            "Ctrl" => mods |= 1,
            "Alt" => mods |= 2,
            "Shift" => mods |= 4,
            "Meta" => mods |= 8,
            "Backspace" if key == 0 => key = 8,
            "F8" if key == 0 => key = 119,
            "F9" if key == 0 => key = 120,
            "F10" if key == 0 => key = 121,
            _ => return Err("Use Ctrl/Alt/Shift/Meta with Backspace, F8, F9, or F10".into()),
        }
    }
    if key == 0 || mods == 0 {
        return Err("The undo shortcut needs a modifier and supported key".into());
    }
    unsafe { cs_shortcut(mods, key) };
    Ok(())
}

pub struct Focus {
    raw: RawSnapshot,
    pub snapshot: Snapshot,
    pub app: String,
}

impl Drop for Focus {
    fn drop(&mut self) {
        unsafe { cs_release(self.raw.handle) }
    }
}

pub fn focus() -> Option<Focus> {
    let mut raw: RawSnapshot = unsafe { std::mem::zeroed() };
    if unsafe { cs_snapshot(&mut raw) } == 0 {
        return None;
    }
    let layout = match raw.layout {
        1 => Layout::English,
        2 => Layout::Russian,
        _ => {
            unsafe { cs_release(raw.handle) };
            return None;
        }
    };
    if raw.length > 192 {
        unsafe { cs_release(raw.handle) };
        return None;
    }
    let suffix = match String::from_utf16(&raw.suffix[..raw.length as usize]) {
        Ok(text) => text,
        Err(_) => {
            unsafe { cs_release(raw.handle) };
            return None;
        }
    };
    let app = unsafe { CStr::from_ptr(raw.app.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    Some(Focus {
        snapshot: Snapshot {
            field: raw.field,
            caret: raw.caret,
            suffix,
            layout,
            revision: raw.revision,
        },
        app,
        raw,
    })
}

impl Focus {
    pub fn replace(&self, action: &Action) -> Result<bool, String> {
        let expected: Vec<u16> = action.expected.encode_utf16().collect();
        let replacement: Vec<u16> = action.replacement.encode_utf16().collect();
        let result = unsafe {
            cs_replace(
                &self.raw,
                expected.as_ptr(),
                expected.len() as u32,
                replacement.as_ptr(),
                replacement.len() as u32,
                match action.target {
                    Layout::English => 1,
                    Layout::Russian => 2,
                },
            )
        };
        match result {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err(
                "The operating system could not complete text replacement. Correction is paused."
                    .into(),
            ),
        }
    }
}
