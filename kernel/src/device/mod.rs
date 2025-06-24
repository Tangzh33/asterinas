// SPDX-License-Identifier: MPL-2.0

mod fb;
mod null;
mod pty;
mod random;
mod shm;
pub mod tty;
mod urandom;
mod zero;
mod event;

#[cfg(all(target_arch = "x86_64", feature = "cvm_guest"))]
mod tdxguest;

use alloc::format;

pub use fb::Fb;
pub use pty::{new_pty_pair, PtyMaster, PtySlave};
pub use random::Random;
pub use urandom::Urandom;
pub use event::EventDevice;

use crate::{
    fs::device::{add_node, Device, DeviceId, DeviceType},
    prelude::*,
};
use alloc::format;

/// Init the device node in fs, must be called after mounting rootfs.
pub fn init() -> Result<()> {
    let null = Arc::new(null::Null);
    add_node(null, "null")?;

    let zero = Arc::new(zero::Zero);
    add_node(zero, "zero")?;

    tty::init();

    let tty = Arc::new(tty::TtyDevice);
    add_node(tty, "tty")?;

    let console = tty::system_console().clone();
    add_node(console, "console")?;

    for (index, tty) in tty::iter_n_tty().enumerate() {
        add_node(tty.clone(), &format!("tty{}", index))?;
    }

    #[cfg(target_arch = "x86_64")]
    ostd::if_tdx_enabled!({
        add_node(Arc::new(tdxguest::TdxGuest), "tdx_guest")?;
    });

    let random = Arc::new(random::Random);
    add_node(random, "random")?;

    let urandom = Arc::new(urandom::Urandom);
    add_node(urandom, "urandom")?;

    let fb = Arc::new(fb::Fb);
    add_node(fb, "fb0")?;
    pty::init()?;

    shm::init()?;

    // Dynamically create EventDevices for each InputDevice
    for (index, (device_name, input_device)) in aster_input::all_devices().iter().enumerate() {
        let event_device = Arc::new(event::EventDevice::new(index, input_device.clone()));
        let path = format!("input/event{}", index);
        add_node(event_device, &path)?;
        println!("Added EventDevice for InputDevice '{}' at '{}'", device_name, path);
    }

    Ok(())
}

// TODO: Implement a more scalable solution for ID-to-device mapping.
// Instead of hardcoding every device numbers in this function,
// a registration mechanism should be used to allow each driver to
// allocate device IDs either statically or dynamically.
pub fn get_device(dev: usize) -> Result<Arc<dyn Device>> {
    if dev == 0 {
        return_errno_with_message!(Errno::EPERM, "whiteout device")
    }

    let devid = DeviceId::from(dev as u64);
    let major = devid.major();
    let minor = devid.minor();

    match (major, minor) {
        (1, 3) => Ok(Arc::new(null::Null)),
        (1, 5) => Ok(Arc::new(zero::Zero)),
        (5, 0) => Ok(Arc::new(tty::TtyDevice)),
        (1, 8) => Ok(Arc::new(random::Random)),
        (1, 9) => Ok(Arc::new(urandom::Urandom)),
        (29, 0) => Ok(Arc::new(fb::Fb)),
        _ => return_errno_with_message!(Errno::EINVAL, "unsupported device"),
    }
}
