use std::ffi::c_void;

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};

// AirPods/Bluetooth headsets in A2DP mode read literal zeros until macOS deigns
// to swap to HFP — and HFP also drops system audio quality to phone-grade.
type AudioObjectID = u32;
type OSStatus = i32;

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        out_data_size: *mut u32,
    ) -> OSStatus;

    fn AudioObjectGetPropertyData(
        object_id: AudioObjectID,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        io_data_size: *mut u32,
        out_data: *mut c_void,
    ) -> OSStatus;
}

const K_AUDIO_OBJECT_SYSTEM_OBJECT: AudioObjectID = 1;
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = fourcc(b"glob");
const K_AUDIO_OBJECT_PROPERTY_SCOPE_INPUT: u32 = fourcc(b"inpt");

const K_AUDIO_HARDWARE_PROPERTY_DEVICES: u32 = fourcc(b"dev#");
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE: u32 = fourcc(b"dIn ");
const K_AUDIO_DEVICE_PROPERTY_TRANSPORT_TYPE: u32 = fourcc(b"tran");
const K_AUDIO_DEVICE_PROPERTY_STREAMS: u32 = fourcc(b"stm#");
const K_AUDIO_OBJECT_PROPERTY_NAME: u32 = fourcc(b"lnam");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BUILT_IN: u32 = fourcc(b"bltn");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_USB: u32 = fourcc(b"usb ");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH: u32 = fourcc(b"blue");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH_LE: u32 = fourcc(b"blea");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_AIRPLAY: u32 = fourcc(b"airp");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRED: u32 = fourcc(b"ccwd");
const K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRELESS: u32 = fourcc(b"ccwl");

const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
}

pub(super) fn pick_reliable_input() -> Option<String> {
    let default_id = read_property_scalar::<AudioObjectID>(
        K_AUDIO_OBJECT_SYSTEM_OBJECT,
        K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )?;
    let default_score = device_score(default_id);
    if default_score == 0 {
        return None;
    }

    log::info!(
        "system default input has unreliable transport (score {default_score}); \
         looking for a more reliable alternative"
    );

    let devices = read_property_array::<AudioObjectID>(
        K_AUDIO_OBJECT_SYSTEM_OBJECT,
        K_AUDIO_HARDWARE_PROPERTY_DEVICES,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )?;
    let mut best: Option<(u8, AudioObjectID)> = None;
    for id in devices {
        if id == default_id || !has_input_streams(id) {
            continue;
        }
        let s = device_score(id);
        if s >= default_score {
            continue;
        }
        match best {
            Some((bs, _)) if bs <= s => {}
            _ => best = Some((s, id)),
        }
    }

    let (score, id) = best?;
    let name = read_device_name(id)?;
    log::info!("preferring more reliable input device (score {score}): {name}");
    Some(name)
}

fn device_score(device_id: AudioObjectID) -> u8 {
    let t = read_property_scalar::<u32>(
        device_id,
        K_AUDIO_DEVICE_PROPERTY_TRANSPORT_TYPE,
        K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
    )
    .unwrap_or(0);
    transport_score(t)
}

fn transport_score(transport: u32) -> u8 {
    match transport {
        K_AUDIO_DEVICE_TRANSPORT_TYPE_BUILT_IN | K_AUDIO_DEVICE_TRANSPORT_TYPE_USB => 0,
        K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_BLUETOOTH_LE
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_AIRPLAY
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRED
        | K_AUDIO_DEVICE_TRANSPORT_TYPE_CONTINUITY_CAPTURE_WIRELESS => 2,
        _ => 1,
    }
}

fn has_input_streams(device_id: AudioObjectID) -> bool {
    let addr = AudioObjectPropertyAddress {
        selector: K_AUDIO_DEVICE_PROPERTY_STREAMS,
        scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_INPUT,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut size: u32 = 0;
    let status =
        unsafe { AudioObjectGetPropertyDataSize(device_id, &addr, 0, std::ptr::null(), &mut size) };
    status == 0 && size > 0
}

fn read_device_name(device_id: AudioObjectID) -> Option<String> {
    let addr = AudioObjectPropertyAddress {
        selector: K_AUDIO_OBJECT_PROPERTY_NAME,
        scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut cf_str: CFStringRef = std::ptr::null();
    let mut size: u32 = std::mem::size_of::<CFStringRef>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut cf_str as *mut _ as *mut c_void,
        )
    };
    if status != 0 || cf_str.is_null() {
        return None;
    }
    let s = unsafe { CFString::wrap_under_create_rule(cf_str) }.to_string();
    Some(s)
}

fn read_property_scalar<T: Default + Copy>(
    object: AudioObjectID,
    selector: u32,
    scope: u32,
) -> Option<T> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut value: T = T::default();
    let mut size: u32 = std::mem::size_of::<T>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut value as *mut _ as *mut c_void,
        )
    };
    if status == 0 {
        Some(value)
    } else {
        None
    }
}

fn read_property_array<T: Default + Copy>(
    object: AudioObjectID,
    selector: u32,
    scope: u32,
) -> Option<Vec<T>> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut size: u32 = 0;
    let status =
        unsafe { AudioObjectGetPropertyDataSize(object, &addr, 0, std::ptr::null(), &mut size) };
    if status != 0 || size == 0 {
        return None;
    }
    let count = size as usize / std::mem::size_of::<T>();
    let mut buf: Vec<T> = vec![T::default(); count];
    let mut size_inout = size;
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &addr,
            0,
            std::ptr::null(),
            &mut size_inout,
            buf.as_mut_ptr() as *mut c_void,
        )
    };
    if status == 0 {
        Some(buf)
    } else {
        None
    }
}
