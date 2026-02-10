#![allow(non_upper_case_globals, non_camel_case_types, dead_code)]

use std::ffi::c_void;

pub type id = *mut c_void;
pub type SEL = *const c_void;
pub type Class = *const c_void;
pub type BOOL = i8;
pub type NSInteger = isize;

pub const NSApplicationActivationPolicyRegular: NSInteger = 0;
pub const NSApplicationActivationPolicyAccessory: NSInteger = 1;
pub const NSApplicationActivationPolicyProhibited: NSInteger = 2;

unsafe extern "C" {
    pub fn objc_getClass(name: *const u8) -> Class;
    pub fn sel_registerName(name: *const u8) -> SEL;
    pub fn objc_msgSend(receiver: id, sel: SEL, ...) -> id;
}

pub unsafe fn msg_send_0(obj: id, sel_name: &[u8]) -> id {
    let sel = unsafe { sel_registerName(sel_name.as_ptr()) };
    unsafe { objc_msgSend(obj, sel) }
}

pub unsafe fn msg_send_1_isize(obj: id, sel_name: &[u8], arg: NSInteger) -> id {
    let sel = unsafe { sel_registerName(sel_name.as_ptr()) };
    unsafe { objc_msgSend(obj, sel, arg) }
}

pub unsafe fn msg_send_1_bool(obj: id, sel_name: &[u8], arg: BOOL) -> id {
    let sel = unsafe { sel_registerName(sel_name.as_ptr()) };
    unsafe { objc_msgSend(obj, sel, arg as std::ffi::c_int) }
}

pub unsafe fn init_nsapp_accessory() {
    let ns_app_class = unsafe { objc_getClass(b"NSApplication\0".as_ptr()) };
    let app = unsafe { msg_send_0(ns_app_class as id, b"sharedApplication\0") };
    unsafe {
        msg_send_1_isize(
            app,
            b"setActivationPolicy:\0",
            NSApplicationActivationPolicyAccessory,
        )
    };
    unsafe { msg_send_0(app, b"finishLaunching\0") };
    eprintln!("NSApplication initialized (Accessory policy, no dock icon)");
}
