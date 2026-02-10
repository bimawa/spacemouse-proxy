#![allow(non_upper_case_globals, dead_code)]

use std::ffi::c_void;

pub type CFAllocatorRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFRunLoopRef = *const c_void;

pub const kCFAllocatorDefault: CFAllocatorRef = std::ptr::null();
pub const kCFStringEncodingUTF8: u32 = 0x08000100;

unsafe extern "C" {
    pub static kCFRunLoopDefaultMode: CFStringRef;

    pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    pub fn CFRunLoopRun();

    pub fn CFStringCreateWithCString(
        allocator: CFAllocatorRef,
        c_string: *const u8,
        encoding: u32,
    ) -> CFStringRef;
}

pub unsafe fn cfstr(s: &[u8]) -> CFStringRef {
    unsafe { CFStringCreateWithCString(kCFAllocatorDefault, s.as_ptr(), kCFStringEncodingUTF8) }
}
