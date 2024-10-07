#![no_std]
#![no_main]
#![allow(warnings)]

mod vmlinux;

use vmlinux::{file, inode, path, vfsmount, dentry, qstr};

use aya_ebpf::{
    helpers::bpf_probe_read_kernel,
    helpers::bpf_probe_read_user,
    helpers::bpf_probe_read,
    helpers::bpf_probe_read_kernel_str_bytes,
    macros::{kprobe,map},
    maps::{HashMap, Array, PerCpuArray},
    programs::ProbeContext,
    
};
use aya_log_ebpf::info;
use core::str::Bytes;
use core::mem::MaybeUninit;

#[map] // 
static INODEDATA: Array<u64> =
    Array::<u64>::with_max_entries(1024, 0);

const MAX_BUFFER_SIZE: usize = 1024;
#[repr(C)]
pub struct Buf {
    pub buf: [u8; 4096],
}

#[map]
pub static mut BUF: PerCpuArray<Buf> = PerCpuArray::with_max_entries(1, 0);

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
    
    // let val : str = check_if_in_directory(&ctx);
    

    let buffer: *const u8 = match try_get_buffer(&ctx){
        Ok(ret) => ret,
        Err(_) => &fail
    };
    let count: usize = match try_get_count(&ctx){
        Ok(ret) => ret,
        Err(_) => 0
    };
    // if(val == 1){
    // let buf : cty::c_long = unsafe {
    //     let ptr = BUF.get_ptr_mut(0).ok_or(0)?;
    //     &mut *ptr
    // };
    // let my_str = unsafe {
    //     core::str::from_utf8_unchecked(bpf_probe_read_kernel_str_bytes(val, &mut buf.buf)?)
    // };
    // unsafe{
    //     info!(&ctx, "Debug val: {}", my_str);
    // }
    // let file : *const file = match ctx.arg(0){
    //     None => return Err(1),
    //     Some(x) => x,
    // };
    
    

    // unsafe {
    //     core::ptr::copy_nonoverlapping(dname.as_ptr(), dir_name.as_mut_ptr(), dname_len);
    // }
    let val : i64 = match try_vfs_write_alt(&ctx){
        Ok(x) => x,
        Err(x) => x,
    };

    // info!(&ctx, "Val: {}", val);
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
    // info!(ctx, "VFS_Write called with buffer size: {}", size);
    return Ok(size);
}
// fn check_if_in_directory(ctx: &ProbeContext) -> str{
    
//     return "";
// }

fn in_dir(file: *const vmlinux::file, dir_inode: u64) -> u32 {
    unsafe{
        //let key: u32 = 0; // Assuming a single key for now
        //let dir_inode = match INODEDATA.get(key) {
        //    Some(inode) => inode,
        //    None => return 0, // If no inode is found, return early
        //};

        //let file: *const vmlinux::file = match ctx.arg(0){
        //    None => return 0,
        //    Some(x) => x,
        //};

        // Read the dentry pointer from the file struct
        let dentry: *const vmlinux::dentry = match bpf_probe_read_kernel(&(*file).f_path.dentry) {
            Ok(dent) => dent,
            Err(_) => return 0, // If reading dentry fails, return early
        };

        let mut current_dentry: *const vmlinux::dentry = dentry;

        // Traverse up the directory structure by following parent dentries
        for _ in 0..10 {  // Max depth of 10 to avoid infinite loops
            if current_dentry.is_null() {
                break;  // Stop if we've reached the root
            }

            // Check if the current dentry's inode matches the directory inode from inodedir map
            let inode: *const vmlinux::inode = match bpf_probe_read_kernel(&(*current_dentry).d_inode) {
                Ok(inode_ptr) => inode_ptr,
                Err(_) => break, // If reading inode fails, stop traversal
            };

            let inode_num: u64 = match bpf_probe_read_kernel(&(*inode).i_ino) {
                Ok(inode_num) => inode_num,
                Err(_) => break, // If reading inode number fails, stop traversal
            };

            if inode_num == dir_inode {
                // Match found, we are in the directory we care about
                return 1;  // Return success
            }

            // Move to the parent directory
            current_dentry = match bpf_probe_read_kernel(&(*current_dentry).d_parent) {
                Ok(parent) => parent,
                Err(_) => break, // If reading parent fails, stop traversal
            };
        }
    }
    0 // Return 0 if no match is found
}


fn try_vfs_write_alt(ctx: &ProbeContext) -> Result<i64, aya_ebpf::cty::c_long> {
    
    //get_file_path(&ctx);
    
    unsafe {
        let key: u32 = 0; // Assuming a single key for now
        let dir_inode = match INODEDATA.get(key) {
            Some(inode) => inode,
            None => return Err(2i64),
        };
        let file: *const vmlinux::file = match ctx.arg(0){
            None => return Err(2i64),
            Some(x) => x,
        };
        if (in_dir(file,*dir_inode) == 0){
            return Ok(0i64);
        }


        let path = bpf_probe_read_kernel(&(*file).f_path)?;
        let dent = path.dentry;
        let inod = match bpf_probe_read_kernel(&(*dent).d_inode){
            Err(x) => {
                info!(ctx, "Error on d_inode {}", x);
                return Err(3i64);
            },
            Ok(x) => x,
        };
        let ino : u64 = bpf_probe_read_kernel(&(*inod).i_ino)?;
        info!(ctx, "path : {}", ino);

    };
    Ok(0i64)
}
fn try_vfs_write(ctx: &ProbeContext) -> Result<i64, aya_ebpf::cty::c_long> {
    /*unsafe {
        let file: *const file = match ctx.arg(0){
            None => return Err(2i64),
            Some(x) => x,
        };
        let inod = bpf_probe_read_kernel(&(*file).f_inode)?;
        let ino : u64 = bpf_probe_read_kernel(&(*inod).i_ino)?;
        info!(ctx, "path : {}", ino);
    }*/
    info!(ctx, "path");
    Ok(0i64)
}
// fn safe_read<T>(ptr: *const T) -> Option<T> {
//     let mut val: MaybeUninit<T> = MaybeUninit::uninit();
//     let ret = bpf_probe_read_kernel(val.as_mut_ptr() as *mut _, core::mem::size_of::<T>(), ptr);
//     if ret.is_ok() {
//         Some(unsafe { val.assume_init() })
//     } else {
//         None
//     }
// }
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}