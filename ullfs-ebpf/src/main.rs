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
use core::str;
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
        for i in 0..50 {  // Max depth of 10 to avoid infinite loops
            
            if current_dentry.is_null() {
                break;  // Stop if we've reached the root
            }

            // Check if the current dentry's inode matches the directory inode from inodedir map
            let inode: *const vmlinux::inode = match bpf_probe_read_kernel(&(*current_dentry).d_inode) {
                Ok(inode_ptr) => inode_ptr,
                Err(_) => break, // If reading inode fails, stop traversal
            };
            let i_num: u64 = match bpf_probe_read_kernel(&(*inode).i_ino) {
                Ok(inode_num) => inode_num,
                Err(_) => break, // If reading inode number fails, stop traversal
            };

            let mut inode_num : u64;
            if (*small_inode == 1){
                inode_num = i_num as u32 as u64;
            } else {
                inode_num = i_num;
            }
            // if (inode_num != 18446612686368067376 && inode_num != 18446612686532053264 && inode_num != 18446612686389248560 && inode_num != 18446612699420303968 && inode_num != 18446612695952264088 && inode_num != 18446612698880437376 && inode_num != 18446612698880437376 && inode_num != 18446612698880438672 && inode_num != 18446612686428750848 && inode_num != 18446612686408229552 && inode_num != 18446612698044543672 && inode_num != 18446612686370013560 && inode_num != 18446612698880433488 && inode_num != 18446612698880433488 && inode_num != 18446612699418977720 && inode_num != 18446612686428762512 && inode_num != 18446612698042823352 && inode_num != 18446612699420306560 && inode_num != 18446612699420297488 && inode_num != 18446612686428756032 && inode_num != 18446612687804570312 && inode_num != 18446612686440151336 && inode_num != 18446612695868191120 && inode_num != 18446612695416620344 && inode_num != 18446612686428759920 && inode_num != 18446612698878807584 && inode_num != 18446612699420298784 && inode_num != 18446612696801438776 && inode_num != 18446612686428752792 && inode_num != 18446612699420296840 && inode_num != 18446612699419220152 && inode_num != 18446612686428754088 && inode_num != 18446612688655885544 && inode_num != 18446612695868183992){
            if (i == 0){
                info!(ctx, "Inode: {} == {}", inode_num, dir_inode);

            }

            if u64::from(inode_num) == dir_inode {
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
        

        if (in_dir(file,*dir_inode, ctx) == 0){
            // info!(ctx, "yeah");
            return Ok(0i64);
        }
        // info!(ctx, "Never gets here");

        
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