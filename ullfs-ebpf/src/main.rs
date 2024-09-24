#![no_std]
#![no_main]

mod binding;

use crate::binding::*;

use aya_ebpf::{
    helpers::bpf_probe_read_kernel,
    macros::{kprobe,map},
    maps::{HashMap, Array},
    programs::ProbeContext,
};
use aya_log_ebpf::info;
use core::str::Bytes;

#[map] // 
static INODEDATA: Array<u32> =
    Array::<u32>::with_max_entries(1024, 0);

const MAX_BUFFER_SIZE: usize = 1024;

#[kprobe]
fn vfs_write(ctx: ProbeContext) -> Result<(), i64> {
    /*
    let file: *mut file = ctx.arg(0).ok_or(1i64)?;
    let tpath = unsafe{(*file).f_path};
    let tdentry = tpath.dentry;

    let inode = unsafe{(*tdentry).d_inode};

    let curnode: u64 = unsafe{(*inode).i_ino};
    //let parentdentry = unsafe{(*dentry).d_parent};

    //

    info!(&ctx, "current node {}", curnode);
    // Get the buffer pointer and count (size of data to write)
    */
    let fail: u8 = 1;
    // let ctx_ref : &ProbeContext = &ctx;

    let buf: *const u8 = match try_get_buffer(&ctx){
        Ok(ret) => ret,
        Err(_) => &fail
    };
    let count: usize = match try_get_count(&ctx){
        Ok(ret) => ret,
        Err(_) => 0
    };
    
    

    Ok(())
}
fn try_get_buffer(ctx: &ProbeContext) -> Result<*const u8, i64>{

    let buf : *const u8 = ctx.arg(1).ok_or(1i64)?;
    return Ok(buf);
    
}
fn try_get_count(ctx: &ProbeContext) -> Result<usize, i64>{
    // let ctx = *ctx_ref;
    let size : usize = ctx.arg(2).ok_or(2i64)?;
    // Ok(size)
    // let sizeStr : u128 = size as u128;
    info!(ctx, "VFS_Write called with buffer size: {}", size);
    return Ok(size);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}