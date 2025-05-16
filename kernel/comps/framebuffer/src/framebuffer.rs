// SPDX-License-Identifier: MPL-2.0

use core::{default, str};

use alloc::sync::Arc;

use ostd::{boot::boot_info, io::IoMem, mm::VmIo, Pod, Result};
use spin::Once;

use crate::{Pixel, PixelFormat, RenderedPixel};

/// The interception of offset for color fileds.
/// 
/// Reference: https://github.com/torvalds/linux/blob/ace4ebf9b70a7daea12102c09ba5ef6bb73223aa/include/uapi/linux/fb.h
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct FrameBufferBitfield {
    /// The beginning of bitfield.
    offset: u32,
    /// The length of bitfield.
    length: u32,
    /// Most significant bit is right(!= 0).
    msb_right: u32,
}

impl Default for FrameBufferBitfield {
    fn default() -> Self {
        Self {
            offset: 0,
            length: 0,
            msb_right: 0,
        }
    }
}

impl FrameBufferBitfield{
    /// Creates a new `FrameBufferBitfield` instance.
    pub fn new(offset: u32, length: u32, msb_right: u32) -> Self {
        Self {
            offset,
            length,
            msb_right,
        }
    }

    /// Returns the offset of the bitfield.
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Returns the length of the bitfield.
    pub fn length(&self) -> u32 {
        self.length
    }

    /// Returns whether the most significant bit is right.
    pub fn msb_right(&self) -> bool {
        self.msb_right != 0
    }
}

/// The framebuffer used for text or graphical output.
///
/// # Notes
///
/// It is highly recommended to use a synchronization primitive, such as a `SpinLock`, to
/// lock the framebuffer before performing any operation on it.
/// Failing to properly synchronize access can result in corrupted framebuffer content
/// or unspecified behavior during rendering.
#[derive(Debug)]
pub struct FrameBuffer {
    io_mem: IoMem,
    base: usize,
    width: usize,
    height: usize,
    pixel_format: PixelFormat,
    red: FrameBufferBitfield,
    green: FrameBufferBitfield,
    blue: FrameBufferBitfield,
    reserved: FrameBufferBitfield,
}

pub static FRAMEBUFFER: Once<Arc<FrameBuffer>> = Once::new();

pub fn get_framebuffer_info() -> Option<Arc<FrameBuffer>> {
    FRAMEBUFFER.get().cloned()
}

pub(crate) fn init() {
    let Some(framebuffer_arg) = boot_info().framebuffer_arg else {
        log::warn!("Framebuffer not found");
        return;
    };

    if framebuffer_arg.address == 0 {
        log::error!("Framebuffer address is zero");
        return;
    }

    // FIXME: There are several pixel formats that have the same BPP. We lost the information
    // during the boot phase, so here we guess the pixel format on a best effort basis.
    let pixel_format = match framebuffer_arg.bpp {
        8 => PixelFormat::Grayscale8,
        16 => PixelFormat::Rgb565,
        24 => PixelFormat::Rgb888,
        32 => PixelFormat::BgrReserved,
        _ => {
            log::error!(
                "Unsupported framebuffer pixel format: {} bpp",
                framebuffer_arg.bpp
            );
            return;
        }
    };

    let framebuffer = {
        let fb_base = framebuffer_arg.address;
        let fb_size = framebuffer_arg.width
            * framebuffer_arg.height
            * (framebuffer_arg.bpp / u8::BITS as usize);
        let io_mem = IoMem::acquire(fb_base..fb_base + fb_size).unwrap();
        FrameBuffer {
            io_mem,
            base: framebuffer_arg.address,
            width: framebuffer_arg.width,
            height: framebuffer_arg.height,
            pixel_format,
            red: FrameBufferBitfield::new(
                framebuffer_arg.red_pos as u32,
                framebuffer_arg.red_size as u32,
                0,
            ),
            green: FrameBufferBitfield::new(
                framebuffer_arg.green_pos as u32,
                framebuffer_arg.green_size as u32,
                0,
            ),
            blue: FrameBufferBitfield::new(
                framebuffer_arg.blue_pos as u32,
                framebuffer_arg.blue_size as u32,
                0,
            ),
            reserved: FrameBufferBitfield::new(
                framebuffer_arg.reserved_pos as u32,
                framebuffer_arg.reserved_size as u32,
                0,
            ),
        }
    };

    framebuffer.clear();
    FRAMEBUFFER.call_once(|| Arc::new(framebuffer));
}

impl FrameBuffer {
    /// Returns the size of the framebuffer in bytes.
    pub fn size(&self) -> usize {
        self.io_mem.length()
    }

    /// Returns the width of the framebuffer in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the framebuffer in pixels.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get the IO memory of the framebuffer.
    pub fn io_mem(&self) -> &IoMem {
        // FIXME: Check the correctness of ownership
        &self.io_mem
    }

    /// Returns the physical address of the framebuffer.
    pub fn io_mem_base(&self) -> usize {
        self.base
    }

    /// Returns the resolution in pixels.
    pub fn resolution(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Returns the number of bytes per pixel (color depth).
    pub fn bytes_per_pixel(&self) -> usize {
        self.pixel_format.nbytes()
    }

    /// Returns the pixel format of the framebuffer.
    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    /// Returns the red color field of the framebuffer.
    pub fn red(&self) -> FrameBufferBitfield {
        self.red
    }

    /// Returns the green color field of the framebuffer.
    pub fn green(&self) -> FrameBufferBitfield {
        self.green
    }

    /// Returns the blue color field of the framebuffer.
    pub fn blue(&self) -> FrameBufferBitfield {
        self.blue
    }

    /// Returns the reserved color field of the framebuffer.
    pub fn reserved(&self) -> FrameBufferBitfield {
        self.reserved
    }

    /// Renders the pixel according to the pixel format of the framebuffer.
    pub fn render_pixel(&self, pixel: Pixel) -> RenderedPixel {
        pixel.render(self.pixel_format)
    }

    /// Calculates the offset of a pixel at the specified position.
    pub fn calc_offset(&self, x: usize, y: usize) -> PixelOffset {
        PixelOffset {
            fb: self,
            offset: ((y * self.width + x) * self.pixel_format.nbytes()) as isize,
        }
    }

    /// Writes a pixel at the specified position.
    pub fn write_pixel_at(&self, offset: PixelOffset, pixel: RenderedPixel) -> Result<()> {
        self.io_mem.write_bytes(offset.as_usize(), pixel.as_slice())
    }

    /// Writes raw bytes at the specified offset.
    pub fn write_bytes_at(&self, offset: usize, bytes: &[u8]) -> Result<()> {
        self.io_mem.write_bytes(offset, bytes)
    }

    /// Clears the framebuffer with default color (black).
    pub fn clear(&self) {
        let frame = alloc::vec![0u8; self.size()];
        self.write_bytes_at(0, &frame).unwrap();
    }
}

/// The offset of a pixel in the framebuffer.
#[derive(Debug, Clone, Copy)]
pub struct PixelOffset<'a> {
    fb: &'a FrameBuffer,
    offset: isize,
}

impl PixelOffset<'_> {
    /// Adds the specified delta to the x coordinate.
    pub fn x_add(&mut self, x_delta: isize) {
        let delta = x_delta * self.fb.pixel_format.nbytes() as isize;
        self.offset += delta;
    }

    /// Adds the specified delta to the y coordinate.
    pub fn y_add(&mut self, y_delta: isize) {
        let delta = y_delta * (self.fb.width * self.fb.pixel_format.nbytes()) as isize;
        self.offset += delta;
    }

    pub fn as_usize(&self) -> usize {
        self.offset as _
    }
}
