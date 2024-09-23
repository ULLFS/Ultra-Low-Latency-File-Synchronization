#![no_std]
use core::{ffi::CStr, result, str::Bytes};

use aya_ebpf::{programs::ProbeContext, maps::HashMap};
use aya::{Bpf, BpfError};
use aya_log_ebpf::info;

#[kprobe]
fn vfs_write(ctx: &KProbeContext) -> Result<(), BpfError> {
    // Get the buffer pointer and count (size of data to write)
    let buf_ptr: *const u8 = ctx.arg(1).try_into()?;
    let count: usize = ctx.arg(2).try_into()?;

    // Ensure we don't exceed the maximum buffer size
    let length = count.min(MAX_BUFFER_SIZE);

    // Prepare a buffer to store the data
    let mut buffer = [0u8; MAX_BUFFER_SIZE];

    // Safely read memory from user space
    let read_length = unsafe {
        core::slice::from_raw_parts(buf_ptr, length).read(&mut buffer[..length])?
    };

    // Get the current process ID
    let pid = ctx.task().pid();


    Ok(())
}