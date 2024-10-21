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
pub static mut BUF: Array<u8> = Array::with_max_entries(4096, 0);
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

unsafe fn in_dir(file: *const vmlinux::file, dir_inode: u64) -> bool {
    //let mut fullLength = 1;
    //
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

            //Updates map then offsets position in map by "len"
            //let len = dnameToMap(current_dentry,&BUF,fullLength);
            //fullLength += (len-1);
            //

            if u64::from(inode_num) == dir_inode {
                // Match found, we are in the directory we care about
                //push_value_to_array(0, (fullLength-1) as u8, &BUF);
                return true;  // Return success
            }

            // Move to the parent directory
            current_dentry = match bpf_probe_read_kernel(&(*current_dentry).d_parent) {
                Ok(parent) => parent,
                Err(_) => break, // If reading parent fails, stop traversal
            };

        }
    }

    //push_value_to_array(0, (fullLength-1) as u8, &BUF);
    false // Return false if no match is found
}

//fills BUFFER at arrayOffset with characters in d_name.name
unsafe fn dnameToMap(dent: *const vmlinux::dentry,array: &Array<u8>, arrayOffset: u32) -> u32 {
    
    let mut end:bool = false;
    let qstring = match bpf_probe_read_kernel(&(*dent).d_name) {
                Ok(q) => q,
                Err(_) => return 0,
            };
    let mut msgLen = qstring.__bindgen_anon_1.__bindgen_anon_1.len;
    for n in 0..4{
        //Limit scope bc name is 64 bytes
        {
            let offset: u32 = n * 64;
            
            //Get qstr name data
            
            
                //With [u8,128] it gives 2 calls stack is 544 too large
            let name: [u8; 64] = match bpf_probe_read_kernel((((qstring).name).add(offset as usize) as *const [u8; 64])) {
                Ok(n) => n,
                Err(_) => return 0,
            };
            
            if msgLen < (n + 1) * 64{
                end = true;
            }

            for i in 0..64{
                if i + n * 64 >= msgLen {
                    // push_value_to_array(n * 64 + arrayOffset + i, 47 as u8, &array);
                    break;
                }
                push_value_to_array(n * 64 + arrayOffset + i, name[i as usize], &array);
            }
            
            //return length
        }
        if end{
            // This is returning exactly msgLen but if I try to return msgLen directly, we run out of instructions
            return n * 64 + msgLen % 64;

            break;
        }
    }

    return 0u32
}

//Just runs dnameToMap n times to get dnames up the directory
//This function is assumes the first element in buffer is length so if that changes it would need updating
unsafe fn pathToMap(dent: *const vmlinux::dentry,array: &Array<u8>, depth: u8) -> bool{
    let mut fullLength = 1;
    
    //This loops just mimics in_dir to search up directories
    let mut current_dentry: *const vmlinux::dentry = dent;
    for i in 0..depth {
        if current_dentry.is_null() {
            break;
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
        
        if u64::from(inode_num) == 2 {
            // Match found, we are in the directory we care about
            //push_value_to_array(0, (fullLength-1) as u8, &BUF);
            break;  // Return success
        }

        //Updates map then offsets position in map by "len"
        let len = dnameToMap(current_dentry,&array,fullLength);
        fullLength += len;

        //Add slash (47)
        push_value_to_array(fullLength, 47 as u8, &array);
        fullLength += 1;
        
        //47 as u8;
        //
        

        current_dentry = match bpf_probe_read_kernel(&(*current_dentry).d_parent) {
            Ok(parent) => parent,
            Err(_) => break, // If reading parent fails, stop traversal
        };
    }

    //Pushes final length to array
    push_value_to_array(0, (fullLength-1) as u8, &array);
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


        /* dnameToMap() Example */
        //Get's current denty name with offset of 1
        //let msgLen = dnameToMap(dent,&BUF,1);
        //Pushes length to 0 Index of buffer for displaying on userside
        //push_value_to_array(0, msgLen as u8, &BUF);

        /* pathToMap() Example */
        //Run's dnameToMap to depth 3 
        pathToMap(dent,&BUF,50);

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