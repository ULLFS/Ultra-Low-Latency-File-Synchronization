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
use aya_ebpf::helpers::bpf_map_update_elem;
use core::ffi::c_void;
use aya_log_ebpf::info;
use core::str;
use core::str::Bytes;
use core::mem::MaybeUninit;

const MAX_BUFFER_SIZE: usize = 1024;

#[map]
pub static mut BUF: Array<u8> = Array::with_max_entries(256, 0);
#[map] // 
static INODEDATA: Array<u64> =
    Array::<u64>::with_max_entries(MAX_BUFFER_SIZE as u32, 0);
#[map]
static PROGDATA: Array<u64> =
    Array::<u64>::with_max_entries(MAX_BUFFER_SIZE as u32,0);




#[kprobe]
fn vfs_write(ctx: ProbeContext) -> Result<(), i64> {
    let fail: u8 = 1;

    let val : i64 = match try_vfs_write(&ctx){
        Ok(x) => x,
        Err(x) => x,
    };
    Ok(())
}

fn in_dir(file: *const vmlinux::file, dir_inode: u64) -> bool {
    unsafe{
        // Read the dentry pointer from the file struct
        let dentry: *const vmlinux::dentry = match bpf_probe_read_kernel(&(*file).f_path.dentry) {
            Ok(dent) => dent,
            Err(_) => return false, // If reading dentry fails, return early
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
            let inode_num: u64 = match bpf_probe_read_kernel(&(*inode).i_ino) {
                Ok(inode_num) => inode_num,
                Err(_) => break, // If reading inode number fails, stop traversal
            };

            if u64::from(inode_num) == dir_inode {
                // Match found, we are in the directory we care about
                return true;  // Return success
            }

            // Move to the parent directory
            current_dentry = match bpf_probe_read_kernel(&(*current_dentry).d_parent) {
                Ok(parent) => parent,
                Err(_) => break, // If reading parent fails, stop traversal
            };

        }
    }
    false // Return false if no match is found
}

//Does not put data in index 0 due to that being reserved for length rn
unsafe fn dnameToMap(dent: *const vmlinux::dentry,array: &Array<u8> ,arrayOffset: u32) -> u32 {
    let mut end:bool = false;
    for n in 0..4{
        //Limit scope bc name is 64 bytes
        {
            //let length = bpf_probe_read_kernel(&(*dent).d_name.__bindgen_anon_1.__bindgen_anon_1.len)?;
            //I can't figure out why length doesn't work it just throws registers
            let qstring = match bpf_probe_read_kernel(&(*dent).d_name) {
                Ok(q) => q,
                Err(_) => return 0, // Return 0 if there's an error
            };

            let offset: usize = n * 64;

            let name: [u8; 64] = match bpf_probe_read_kernel((((qstring).name).add(offset) as *const u8 as *const [u8; 64])) {
                Ok(n) => n,
                Err(_) => return 0, // Return 0 if there's an error
            };
            //With [u8,128] it gives 2 calls stack is 544 too large
    
            let mut getLen: u32 = 0;
            for i in 0..64 {
                getLen += 1;
                if name[i] == 0 {
                    end = true;
                    break;
                }
            }

            for i in 0..getLen{
                push_value_to_array(((n*64) + (1 + arrayOffset) as usize + i as usize) as u32, name[i as usize], &array);
            }
            //The length for printing on user size
            if end {
                //push_value_to_array(0, ((n*64)+getLen as usize) as u8, &array);
                return ((n*64)+getLen as usize) as u32;
            }
        }
        if end{
            break;
        }
    }
    return 0u32
}

unsafe fn getDentryDepth(dent: *const vmlinux::dentry) -> u8 {
    let mut depth: u8 = 0; // Initialize depth to 0
    let mut current_dentry: *const vmlinux::dentry = dent;

    // Loop for a maximum of MAX_DEPTH iterations
    for i in 0..10 {
        if current_dentry.is_null() {
            return i; // Stop if we've reached the root or a null pointer
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
        if inode_num == 2 {
            // Match found, we are in the directory we care about
            return i;  // Return success
        }

        // Read the parent dentry
        current_dentry = match bpf_probe_read_kernel(&(*current_dentry).d_parent) {
            Ok(parent) => parent,
            Err(_) => break, // If reading parent fails, stop traversal
        };

        // Increment depth
        depth += 1;
    }

    // Return the depth
    depth
}

unsafe fn pathToBuffer(dent: *const vmlinux::dentry,array: &Array<u8>, depth: u8) -> bool{
    let mut fullLength = 0;
        
    let mut current_dentry: *const vmlinux::dentry = dent;
    for i in 0..depth {
        if current_dentry.is_null() {
            break;
        }
        let len = dnameToMap(current_dentry,&BUF,fullLength);
        fullLength += len-1;

        current_dentry = match bpf_probe_read_kernel(&(*current_dentry).d_parent) {
            Ok(parent) => parent,
            Err(_) => break, // If reading parent fails, stop traversal
        };
    }
    push_value_to_array(0, fullLength as u8, &BUF);
    return true;
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
        

        if (!in_dir(file,*dir_inode)){
            // info!(ctx, "yeah");
            push_value_to_array(0, 0u8, &BUF);
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
        
        //info!(ctx, "Error on d_inode {}", 2);
        //if (length > 10){
        //    return Ok(0i64);
        //}
        //let mut my_str = [0u8; 8];
        // info!(ctx, "TEST");
        
        //this get's the depth but if added to pathToBuffer it causes the compiler to precompute
        //  too many instructions
        //let depth:u8 = getDentryDepth(dent);

        //let capped_depth = if depth > 4 { 4 } else { depth };

        //push_value_to_array(0, depth, &BUF);
        
        //if depth is more than steps to root it breaks
        //  and I cannot use a variable because that makes it
        //  calculate over a million instructions and it breaks
        pathToBuffer(dent,&BUF,3);
        
        /*for i in 1..(length - 1){
            if name == 0 {

            }
            push_value_to_array(i, name[i as usize], &BUF);
        }*/
        //push_value_to_array(2, name[1], &BUF);
        //push_value_to_array(3, name[2], &BUF);
        //push_value_to_array(4, name[3], &BUF);
        // push_value_to_array(i + 1, name[(i + 1) as usize], &BUF);
        // push_value_to_array(i + 1, name[(i + 1) as usize], &BUF);
        // push_value_to_array(i + 1, name[(i + 1) as usize], &BUF);
        // push_value_to_array(0, length, &BUF);
        
        // info!(ctx, "path : {}",my_str);

    };
    Ok(0i64)
}

unsafe fn push_value_to_array<T:Copy>(index: u32, value: T, array : &Array<T>){
    let output_data_ptr: *mut aya_ebpf::maps::Array<T> = array as *const _ as *mut aya_ebpf::maps::Array<T>;

    bpf_map_update_elem(
        output_data_ptr as *mut c_void, // Map pointer
        &index as *const u32 as *const c_void, // Pointer to the key
        &value as *const T as *const c_void, // Pointer to the value
        0, // Flags
    );
}
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}