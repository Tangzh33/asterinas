// SPDX-License-Identifier: MPL-2.0

//! The console device of Asterinas.

#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

pub mod font;
pub mod mode;

use alloc::{collections::BTreeMap, fmt::Debug, string::String, sync::Arc, vec::Vec};
use core::any::Any;

use component::{init_component, ComponentInitError};
use ostd::{
    mm::{Infallible, VmReader},
    sync::{LocalIrqDisabled, SpinLock, SpinLockGuard},
};
use spin::Once;

pub type ConsoleCallback = dyn Fn(VmReader<Infallible>) + Send + Sync;

/// An error returned by [`AnyConsoleDevice::set_font`].
#[derive(Debug, Clone, Copy)]
pub enum ConsoleSetFontError {
    InappropriateDevice,
    InvalidFont,
}

pub trait AnyConsoleDevice: Send + Sync + Any + Debug {
    /// Sends data to the console device.
    fn send(&self, buf: &[u8]);

    /// Registers a callback that will be invoked when the console device receives data.
    ///
    /// The callback may be called in the interrupt context. Therefore, it should _never_ sleep.
    fn register_callback(&self, callback: &'static ConsoleCallback);

    /// Sets the font of the console device.
    fn set_font(&self, _font: font::BitmapFont) -> Result<(), ConsoleSetFontError> {
        Err(ConsoleSetFontError::InappropriateDevice)
    }

    // TODO: Add support for getting the font of the console device.

    /// Sets the console mode (text or graphics, see [`mode::ConsoleMode`]).
    ///
    /// Returns true if the mode was changed, false if the mode is not supported.
    #[must_use]
    fn set_mode(&self, _mode: mode::ConsoleMode) -> bool {
        false
    }

    /// Gets the current console mode.
    ///
    /// Returns the current console mode, or `None` if mode switching is not supported.
    fn mode(&self) -> Option<mode::ConsoleMode> {
        None
    }

    /// Sets the keyboard mode (see [`mode::KeyboardMode`]).
    ///
    /// Returns true if the mode was changed, false if the mode is not supported.
    #[must_use]
    fn set_keyboard_mode(&self, _mode: mode::KeyboardMode) -> bool {
        false
    }

    /// Gets the current keyboard mode.
    ///
    /// Returns the current keyboard mode, or `None` if mode switching is not supported.
    fn keyboard_mode(&self) -> Option<mode::KeyboardMode> {
        None
    }
}

pub const FRAMEBUFFER_CONSOLE_NAME: &str = "Framebuffer-Console";
pub const FAKE_CONSOLE_NAME: &str = "Fake-Console";

pub fn register_device(name: String, device: Arc<dyn AnyConsoleDevice>) {
    COMPONENT
        .get()
        .unwrap()
        .console_device_table
        .lock()
        .insert(name, device);
}

pub fn all_devices() -> Vec<(String, Arc<dyn AnyConsoleDevice>)> {
    let console_devices = COMPONENT.get().unwrap().console_device_table.lock();
    let mut devices: Vec<_> = console_devices
        .iter()
        .map(|(name, device)| (name.clone(), device.clone()))
        .collect();

    if devices.len() == 1 && devices[0].0 == FRAMEBUFFER_CONSOLE_NAME {
        devices.insert(0, (String::from(FAKE_CONSOLE_NAME), fake_console()));
    }

    devices
}

pub fn all_devices_lock<'a>(
) -> SpinLockGuard<'a, BTreeMap<String, Arc<dyn AnyConsoleDevice>>, LocalIrqDisabled> {
    COMPONENT.get().unwrap().console_device_table.lock()
}

static COMPONENT: Once<Component> = Once::new();
static FAKE_CONSOLE: Once<Arc<dyn AnyConsoleDevice>> = Once::new();

fn fake_console() -> Arc<dyn AnyConsoleDevice> {
    FAKE_CONSOLE.call_once(|| Arc::new(FakeConsole) as Arc<dyn AnyConsoleDevice>);
    FAKE_CONSOLE.get().unwrap().clone()
}

#[init_component]
fn component_init() -> Result<(), ComponentInitError> {
    let component = Component::init()?;
    COMPONENT.call_once(|| component);
    Ok(())
}

#[derive(Debug)]
struct Component {
    console_device_table: SpinLock<BTreeMap<String, Arc<dyn AnyConsoleDevice>>, LocalIrqDisabled>,
}

impl Component {
    pub fn init() -> Result<Self, ComponentInitError> {
        Ok(Self {
            console_device_table: SpinLock::new(BTreeMap::new()),
        })
    }
}

#[derive(Debug)]
struct FakeConsole;

impl AnyConsoleDevice for FakeConsole {
    fn send(&self, _buf: &[u8]) {}

    fn register_callback(&self, _callback: &'static ConsoleCallback) {}
}
