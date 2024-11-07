// SPDX-License-Identifier: MPL-2.0

#![no_std]
#![deny(unsafe_code)]

use ostd::prelude::*;
use ostd::mm::page::allocator::PageAlloc;
use alloc::boxed::Box;

extern crate alloc;

#[ostd::main]
fn kernel_main() {
    let avail_mem_as_mb = mylib::available_memory() / 1_000_000;
    println!("The available memory is {} MB", avail_mem_as_mb);
}

#[ostd::page_allocator_init_fn]
fn init_page_allocator() -> Option<Box<dyn PageAlloc>> {
    None
}