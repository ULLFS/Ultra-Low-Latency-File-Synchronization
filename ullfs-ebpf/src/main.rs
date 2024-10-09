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
    helpers::bpf_probe_read_kernel_str,
    macros::{kprobe,map},
    maps::{HashMap, Array, PerCpuArray},
    programs::ProbeContext,
    
};
use aya_log_ebpf::info;
use core::str::Bytes;
use core::mem::MaybeUninit;

const MAX_BUFFER_SIZE: usize = 1024;

#[map] // 
static INODEDATA: Array<u64> =
    Array::<u64>::with_max_entries(MAX_BUFFER_SIZE as u32, 0);
#[map]
static PROGDATA: Array<u64> =
    Array::<u64>::with_max_entries(MAX_BUFFER_SIZE as u32,0);


#[repr(C)]
pub struct Buf {
    pub buf: [u8; 4096],
}

#[map]
pub static mut BUF: PerCpuArray<Buf> = PerCpuArray::with_max_entries(1, 0);

#[kprobe]
fn vfs_write(ctx: ProbeContext) -> Result<(), i64> {
    let fail: u8 = 1;

    let val : i64 = match try_vfs_write(&ctx){
        Ok(x) => x,
        Err(x) => x,
    };
    Ok(())
}

fn in_dir(file: *const vmlinux::file, dir_inode: u64) -> u32 {
    unsafe{
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


fn try_vfs_write(ctx: &ProbeContext) -> Result<i64, aya_ebpf::cty::c_long> {
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

        if (dent.is_null()){
            return Ok(0i64);
        }
    
        let length: u32 = bpf_probe_read_kernel(&(*dent).d_name.__bindgen_anon_1.__bindgen_anon_1.len)?;
        //if (length > 10){
        //    return Ok(0i64);
        //}
        //let mut my_str = [0u8; 8];
        //let qstring: ::aya_ebpf::cty::c_uchar = bpf_probe_read_kernel((*dent).d_name.name)?;
        //bpf_probe_read_kernel_str(&qstring, &mut my_str)?;
        // for i in 0..length{

        // }
        info!(ctx, "path : {}", length);

    };
    Ok(0i64)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}