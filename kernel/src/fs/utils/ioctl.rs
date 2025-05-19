// SPDX-License-Identifier: MPL-2.0

use crate::prelude::*;

#[repr(u32)]
#[derive(Debug, Clone, Copy, TryFromInt)]
pub enum IoctlCmd {
    /// Get terminal attributes
    TCGETS = 0x5401,
    TCSETS = 0x5402,
    /// Drain the output buffer and set attributes
    TCSETSW = 0x5403,
    /// Drain the output buffer, and discard pending input, and set attributes
    TCSETSF = 0x5404,
    /// Make the given terminal the controlling terminal of the calling process.
    TIOCSCTTY = 0x540e,
    /// Get the process group ID of the foreground process group on this terminal
    TIOCGPGRP = 0x540f,
    /// Set the foreground process group ID of this terminal.
    TIOCSPGRP = 0x5410,
    /// Get the number of bytes in the input buffer.
    FIONREAD = 0x541B,
    /// Set window size
    TIOCGWINSZ = 0x5413,
    TIOCSWINSZ = 0x5414,
    /// Enable or disable non-blocking I/O mode.
    FIONBIO = 0x5421,
    /// the calling process gives up this controlling terminal
    TIOCNOTTY = 0x5422,
    /// Return the session ID of FD
    TIOCGSID = 0x5429,
    /// Clear the close on exec flag on a file descriptor
    FIONCLEX = 0x5450,
    /// Set the close on exec flag on a file descriptor
    FIOCLEX = 0x5451,
    /// Enable or disable asynchronous I/O mode.
    FIOASYNC = 0x5452,
    /// Get Pty Number
    TIOCGPTN = 0x80045430,
    /// Lock/unlock Pty
    TIOCSPTLCK = 0x40045431,
    /// Safely open the slave
    TIOCGPTPEER = 0x40045441,
    /// Get tdx report using TDCALL
    TDXGETREPORT = 0xc4405401,
    /// Get variable screen information (resolution, pixel format, etc.)
    ///
    /// Args(arg: usize):
    /// - A pointer to a [`FbVarScreenInfo`] structure
    /// - Output-only
    /// - Kernel fills the struct with current variable screen settings
    FBIOGETVSCREENINFO = 0x4600,
    /// Set variable screen information (adjust display parameters)
    ///
    /// Args(arg: usize):
    /// - A pointer to a [`FbVarScreenInfo`] structure
    /// - **Both** input and output
    /// - Input: user-provided settings
    /// - Output: kernel returns updated/validated settings
    FBIOPUTVSCREENINFO = 0x4601,
    /// Get fixed/static screen information (memory layout, driver name)
    ///
    /// Args(arg: usize):
    /// - A pointer to a [`FbFixScreenInfo`] structure
    /// - Output-only
    /// - Kernel provides unchangeable hardware/driver details
    FBIOGETFSCREENINFO = 0x4602,
    /// Get color palette (color map) from the framebuffer
    ///
    /// Args(arg: usize):
    /// - A pointer to a [`FbCmap`] structure
    /// - Output-only
    /// - Kernel writes current color map data into the struct
    FBIOGETCMAP = 0x4604,
    /// Set color palette (color map) for the framebuffer
    ///
    /// Args(arg: usize):
    /// - A pointer to a [`FbCmap`] structure
    /// - Input-only
    /// - Kernel applies the provided map
    FBIOPUTCMAP = 0x4605,
    /// Pan or wrap the visible portion of the display buffer
    ///
    /// Args(arg: usize):
    /// - A pointer to a [`FbVarScreenInfo`] structure
    /// - **Both** input and output
    /// - Input: new panning offsets
    /// - Output: kernel returns adjusted offsets
    FBIOPANDISPLAY = 0x4606,
    /// Blank or unblank the screen (turn display on/off)
    ///
    /// Args(arg: usize):
    /// - An Int value representing the blanking mode
    ///   - 0: screen: unblanked, hsync: on,  vsync: on
    ///   - 1: screen: blanked,   hsync: on,  vsync: on
    ///   - 2: screen: blanked,   hsync: on,  vsync: off
    ///   - 3: creen: blanked,   hsync: off, vsync: on
    ///   - 4: screen: blanked,   hsync: off, vsync: off
    /// - Input-only
    /// - Kernel uses the value to control the display state
    FBIOBLANK = 0x4611,
}
