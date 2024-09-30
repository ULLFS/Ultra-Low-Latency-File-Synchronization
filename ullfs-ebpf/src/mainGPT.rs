#![no_std]
#![feature(llvm_asm)]

use aya_ebpf::{programs::ProbeContext, macros::{kprobe,map}, Program};
use aya_ebpf::maps::{HashMap, PerfMap};
use core::mem::size_of;
use bindings::*;
const MAX_DIR_NAME_LEN: usize = 128;

#[allow(dead_code)] // This may be needed in your bindings.rs
mod bindings {
    // Include your structs representing file, dentry, inode, sb, dname
}

#[aya_ebpf::macros::map]
pub struct MyMap {
    #[map(type = HashMap)]
    pub map: HashMap<u32, u64>,
}

#[aya_ebpf::macros::program]
pub fn kprobe_vfs_write(ctx: ProbeContext) -> Result<(), aya_ebpf::bindings::bpf_errors::BPF_ERROR> {
    

    Ok(())
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::intrinsics::abort() }
}

