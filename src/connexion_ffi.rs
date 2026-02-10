#![allow(non_upper_case_globals, non_camel_case_types, dead_code)]

use std::ffi::c_void;

pub const kConnexionClientWildcard: u32 = 0x2A2A2A2A;
pub const kConnexionClientManual: u32 = 0x2B2B2B2B;
pub const kConnexionClientModeTakeOver: u16 = 1;
pub const kConnexionClientModePlugin: u16 = 2;

pub const kConnexionCtlActivateClient: u32 = u32::from_be_bytes(*b"3dac");
pub const kConnexionCtlDeactivateClient: u32 = u32::from_be_bytes(*b"3ddc");

pub const kConnexionMaskAll: u32 = 0x3FFF;
pub const kConnexionMaskAllButtons: u32 = 0xFFFFFFFF;

pub const kConnexionMsgDeviceState: u32 = u32::from_be_bytes(*b"3dSR");
pub const kConnexionMsgPrefsChanged: u32 = u32::from_be_bytes(*b"3dPC");

pub const kConnexionCmdHandleRawData: u16 = 1;
pub const kConnexionCmdHandleButtons: u16 = 2;
pub const kConnexionCmdHandleAxis: u16 = 3;

#[repr(C, packed(2))]
#[derive(Debug, Clone, Copy)]
pub struct ConnexionDeviceState {
    pub version: u16,
    pub client: u16,
    pub command: u16,
    pub param: i16,
    pub value: i32,
    pub time: u64,
    pub report: [u8; 8],
    pub buttons8: u16,
    pub axis: [i16; 6],
    pub address: u16,
    pub buttons: u32,
}

pub type ConnexionAddedHandlerProc = extern "C" fn(product_id: u32);
pub type ConnexionRemovedHandlerProc = extern "C" fn(product_id: u32);
pub type ConnexionMessageHandlerProc =
    extern "C" fn(product_id: u32, message_type: u32, message_argument: *mut c_void);

unsafe extern "C" {
    pub fn SetConnexionHandlers(
        message_handler: ConnexionMessageHandlerProc,
        added_handler: ConnexionAddedHandlerProc,
        removed_handler: ConnexionRemovedHandlerProc,
        use_separate_thread: bool,
    ) -> i16;

    pub fn CleanupConnexionHandlers();

    pub fn RegisterConnexionClient(signature: u32, name: *const u8, mode: u16, mask: u32) -> u16;

    pub fn SetConnexionClientButtonMask(client_id: u16, button_mask: u32);

    pub fn UnregisterConnexionClient(client_id: u16);

    pub fn ConnexionClientControl(
        client_id: u16,
        message: u32,
        param: i32,
        result: *mut i32,
    ) -> i16;
}
