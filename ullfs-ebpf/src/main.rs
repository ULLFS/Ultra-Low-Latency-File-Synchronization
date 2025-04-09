#![no_std]
#![no_main]
#![allow(warnings)]

mod vmlinux;

use vmlinux::{file, inode, path, vfsmount, renamedata, dentry, qstr};

use aya_ebpf::{
    helpers::bpf_probe_read_kernel,
    helpers::bpf_probe_read_user,
    helpers::bpf_probe_read,
    helpers::bpf_probe_read_kernel_str_bytes,
    helpers::bpf_probe_read_kernel_str,
    helpers::gen::bpf_send_signal,
    macros::{kprobe,kretprobe,fexit,fentry,map},
    maps::{HashMap, Array, PerCpuArray},
    programs::ProbeContext,
    programs::FExitContext,
    programs::FEntryContext,
};

use aya_ebpf::{cty::c_int, macros::lsm, programs::LsmContext};
use aya_ebpf::EbpfContext;

use aya_ebpf::helpers::bpf_map_update_elem;
use core::ffi::c_void;
use core::arch::asm;
use aya_log_ebpf::info;
use core::str;
use core::str::Bytes;
use core::mem::MaybeUninit;
use aya_ebpf::maps::perf::PerfEventArray;
const MAX_BUFFER_SIZE: usize = 1024;

// ========================================
// =                MAPS                  =
// ========================================

#[repr(C)]
struct EventData {
    inod: u64,
    inod2: u64,
    len: u16,        // Length (2 bytes)
    len2: u16,
    event_type: u8,  // The type of the event (1 byte)

    //0 is unused
    //1 is both in watch dir
    //2 is to in watch dir [Move to from outside]
    //3 is from in watch dir [Moved out of watch dir]
    //4 is neither [Impossible]
    rename_state: u8,

}

#[map]
pub static mut BUF: PerCpuArray<[u8; 64]> = PerCpuArray::with_max_entries(4096, 0);

#[map]
pub static mut BUFTWO: PerCpuArray<[u8; 64]> = PerCpuArray::with_max_entries(4096, 0);

#[map] 
static INODEDATA: Array<u64> =
    Array::<u64>::with_max_entries(MAX_BUFFER_SIZE as u32, 0);
#[map]
static PROGDATA: Array<u64> =
    Array::<u64>::with_max_entries(MAX_BUFFER_SIZE as u32,0);
#[map]
static EVENTS:  PerfEventArray<EventData> = 
    PerfEventArray::<EventData>::new(0);


// ========================================
// =              LSM HOOKS               =
// ========================================

// LSM_HOOK(int, 0, path_unlink, const struct path *dir, struct dentry *dentry)
#[lsm(hook = "path_unlink")]
pub fn path_unlink(ctx: LsmContext) -> i32 {
    // Assume path parameter at index 0
    match unsafe { try_arg_path(ctx, 0, 0) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_unlink, const struct path *dir, struct dentry *dentry)
#[lsm(hook = "path_mkdir")]
pub fn path_mkdir(ctx: LsmContext) -> i32 {
    // Assume path parameter at index 0
    match unsafe { try_arg_path(ctx, 0, 1) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_mkdir, const struct path *dir, struct dentry *dentry,umode_t mode)
#[lsm(hook = "path_rmdir")]
pub fn path_rmdir(ctx: LsmContext) -> i32 {
    // Assume path parameter at index 0
    match unsafe { try_arg_dentry(ctx, 1, 2) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_mknod, const struct path *dir, struct dentry *dentry, umode_t mode, unsigned int dev)
#[lsm(hook = "path_mknod")]
pub fn path_mknod(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_path(ctx, 0, 3) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_symlink, const struct path *dir, struct dentry *dentry,const char *old_name)
#[lsm(hook = "path_symlink")]
pub fn path_symlink(ctx: LsmContext) -> i32 {
    // Assume path parameter at index 0
    match unsafe { try_arg_path(ctx, 0, 4) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_link, struct dentry *old_dentry,const struct path *new_dir, struct dentry *new_dentry)
#[lsm(hook = "path_link")]
pub fn path_link(ctx: LsmContext) -> i32 {
    // Assume dentry parameter at index 0
    match unsafe { try_arg_path(ctx, 1, 5) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_rename, const struct path *old_dir, struct dentry *old_dentry, const struct path *new_dir, struct dentry *new_dentry, unsigned int flags)
#[lsm(hook = "path_rename")]
pub fn path_rename(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_path(ctx, 2, 6) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_notify, const struct path *path, u64 mask, unsigned int obj_type)
#[lsm(hook = "path_notify")]
pub fn path_notify(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_path(ctx, 0, 7) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_chmod, const struct path *path, umode_t mode)
#[lsm(hook = "path_chmod")]
pub fn path_chmod(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_path(ctx, 0, 8) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_chown, const struct path *path, kuid_t uid, kgid_t gid)
#[lsm(hook = "path_chown")]
pub fn path_chown(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_path(ctx, 0, 9) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, path_chroot, const struct path *path)
#[lsm(hook = "path_chroot")]
pub fn path_chroot(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_path(ctx, 0, 10) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, inode_create, struct inode *dir, struct dentry *dentry,umode_t mode)
#[lsm(hook = "inode_create")]
pub fn inode_create(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_dentry_parent(ctx, 1, 11) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, inode_link, struct dentry *old_dentry, struct inode *dir,struct dentry *new_dentry)
#[lsm(hook = "inode_link")]
pub fn inode_link(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_dentry_parent(ctx, 0, 12) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, inode_unlink, struct inode *dir, struct dentry *dentry)
#[lsm(hook = "inode_unlink")]
pub fn inode_unlink(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_dentry(ctx, 1, 13) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, inode_symlink, struct inode *dir, struct dentry *dentry,const char *old_name)
#[lsm(hook = "inode_symlink")]
pub fn inode_symlink(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_dentry_parent(ctx, 1, 14) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, inode_mkdir, struct inode *dir, struct dentry *dentry,umode_t mode)
#[lsm(hook = "inode_mkdir")]
pub fn inode_mkdir(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_dentry_parent(ctx, 1, 15) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, inode_rmdir, struct inode *dir, struct dentry *dentry)
#[lsm(hook = "inode_rmdir")]
pub fn inode_rmdir(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_dentry(ctx, 1, 16) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, inode_mknod, struct inode *dir, struct dentry *dentry,umode_t mode, dev_t dev)
#[lsm(hook = "inode_mknod")]
pub fn inode_mknod(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_dentry_parent(ctx, 1, 17) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(int, 0, inode_rename, struct inode *old_dir, struct dentry *old_dentry,struct inode *new_dir, struct dentry *new_dentry)
#[lsm(hook = "inode_rename")]
pub fn inode_rename(ctx: LsmContext) -> i32 {
    match unsafe { try_inode_rename(&ctx, 18) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// LSM_HOOK(void, LSM_RET_VOID, d_instantiate, struct dentry *dentry,struct inode *inode)
#[lsm(hook = "d_instantiate")]
pub fn d_instantiate(ctx: LsmContext) -> i32 {
    // Assume dentry parameter is at argument index 0
    match unsafe { try_arg_inode_dentry(ctx, 1, 0, 19)} {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}


#[lsm(hook = "inode_setattr")]
pub fn inode_setattr(ctx: LsmContext) -> i32 {
    match unsafe { try_arg_dentry(ctx, 1, 31) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}


fn try_arg_file(ctx: LsmContext, path_arg_pos: u8,calltype: u8) -> Result<(), i64> {
    unsafe{
        let file: *const vmlinux::file = ctx.arg(path_arg_pos as usize);
        let path = bpf_probe_read_kernel(&(*file).f_path)?;
        let dent: *const vmlinux::dentry = path.dentry;
        
        let val : i64 = match try_dentry(&ctx, dent, calltype){
            Ok(x) => x,
            Err(x) => x,
        };
        Ok(())
    }
}

fn try_arg_path(ctx: LsmContext, path_arg_pos: u8,calltype: u8) -> Result<(), i64> {
    unsafe{
        let path: *const vmlinux::path = ctx.arg(path_arg_pos as usize);
        let dent: *const vmlinux::dentry = bpf_probe_read_kernel(&(*path).dentry)?;
        
        let val : i64 = match try_dentry(&ctx, dent, calltype){
            Ok(x) => x,
            Err(x) => x,
        };
        Ok(())
    }
}

fn try_arg_dentry(ctx: LsmContext, dent_arg_pos: u8,calltype: u8) -> Result<(), i64> {
    unsafe{
        let entry: *const vmlinux::dentry = ctx.arg(dent_arg_pos as usize);
        
        let val : i64 = match try_dentry(&ctx, entry, calltype){
            Ok(x) => x,
            Err(x) => x,
        };
        Ok(())
    }
}

fn try_arg_dentry_parent(ctx: LsmContext, dent_arg_pos: u8,calltype: u8) -> Result<(), i64> {
    unsafe{
        let entry: *const vmlinux::dentry = ctx.arg(dent_arg_pos as usize);
        let dent: *const vmlinux::dentry = bpf_probe_read_kernel(&(*entry).d_parent)?;
        
        let val : i64 = match try_dentry(&ctx, dent, calltype){
            Ok(x) => x,
            Err(x) => x,
        };
        Ok(())
    }
}

fn try_arg_inode_dentry(ctx: LsmContext, inode_arg_pos: u8, dent_arg_pos: u8, calltype: u8) -> Result<(), i64> {
    unsafe{
        let entry: *const vmlinux::dentry = ctx.arg(dent_arg_pos as usize);
        let dent: *const vmlinux::dentry = bpf_probe_read_kernel(&(*entry).d_parent)?;

        let dir_inode = match INODEDATA.get(0) {
            Some(inode) => inode,
            None => &0,
        };

        if (!in_dir(dent,*dir_inode)){
            push_value_to_array(0, [0u8; 64], &BUF);
            return Ok(());
        }

        let inode: *const vmlinux::inode = ctx.arg(inode_arg_pos as usize);
        // let inode: *const vmlinux::inode = bpf_probe_read_kernel(&(*dent).d_inode)?;
        let inode_num: u64 = bpf_probe_read_kernel(&(*inode).i_ino)?;

        let len = pathToMap(dent,&BUF, 50, &ctx, *dir_inode);
        let event_data = EventData {
            inod: inode_num,
            inod2: 0u64,
            len: len as u16, // Existing length value, cast to u16
            len2: 0u16,
            event_type: calltype,   // Example event type (you can change it based on your use case)
            rename_state: 0u8,
        };
        //EVENTS.output(ctx, &(len as u16), 0);
        EVENTS.output(&ctx, &event_data, 0);
        Ok(())
    }
}

fn try_arg_empty_event(ctx: LsmContext,calltype: u8) -> Result<(), i64> {
    unsafe{
        let event_data = EventData {
            event_type: calltype,   // Example event type (you can change it based on your use case)
            len: 0u16, // Existing length value, cast to u16
            inod: 0,
            len2: 0u16,
            inod2: 0u64,
            rename_state: 0u8,
        };
        EVENTS.output(&ctx, &event_data, 0);
        Ok(())
    }
}

fn try_inode_rename(ctx: &LsmContext, calltype: u8) -> Result<(), i64> {
    unsafe {
        
        let key: u32 = 0; // Assuming a single key for now
        let dir_inode = match INODEDATA.get(key) {
            Some(inode) => inode,
            None => return Err(2i64),
        };
        
        //FromPath
        let dent: *const vmlinux::dentry = ctx.arg(1 as usize);

        let mut len = 0;
        let mut inode_num: u64 = 0;

        let mut rn_state: u8 = 1;

        let inode: *const vmlinux::inode = bpf_probe_read_kernel(&(*dent).d_inode)?;
        inode_num = bpf_probe_read_kernel(&(*inode).i_ino)?;

        if (in_dir(dent,*dir_inode)){
            len = pathToMap(dent,&BUF, 50, ctx, *dir_inode);
        }
        else{
            rn_state = 2;
            push_value_to_array(0, [0u8; 64], &BUF);
        }


        //ToPath
        let entry: *const vmlinux::dentry = ctx.arg(3 as usize);
        let second_dent: *const vmlinux::dentry = bpf_probe_read_kernel(&(*entry).d_parent)?;

        let mut len2 = 0;
        let mut inode_num2: u64 = 0;

        let inode2: *const vmlinux::inode = bpf_probe_read_kernel(&(*second_dent).d_inode)?;
        inode_num2 = bpf_probe_read_kernel(&(*inode2).i_ino)?;

        if (in_dir(second_dent,*dir_inode)){
            len2 = pathToMap(second_dent,&BUFTWO, 50, ctx, *dir_inode);
        }
        else{
            if(rn_state == 2){
                return Ok(());
            }
            else{
                rn_state = 3;
            }
            push_value_to_array(0, [0u8; 64], &BUFTWO);
        }

        //event
        let event_data = EventData {
            inod: inode_num as u64, 
            inod2: inode_num2 as u64,
            len: len as u16, // Existing length value, cast to u16
            len2: len2 as u16,
            event_type: calltype as u8,   // Example event type (you can change it based on your use case)
            rename_state: rn_state as u8,
        };
        //EVENTS.output(ctx, &(len as u16), 0);
        EVENTS.output(ctx, &event_data, 0);
    };
    Ok(())
}


// ========================================
// =               FEXIT                  =
// ========================================

// ssize_t vfs_iter_write(struct file *file, struct iov_iter *iter, loff_t *ppos,rwf_t flags)
#[fexit(function = "vfs_iter_write")]
fn vfs_iter_write(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_file(ctx, 0, 20) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

//int vfs_mkdir(struct mnt_idmap *idmap, struct inode *dir, struct dentry *dentry, umode_t mode)
#[fexit(function = "vfs_mkdir")]
fn vfs_mkdir(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_dentry(ctx, 2, 21) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// int vfs_mknod(struct mnt_idmap *, struct inode *, struct dentry *,umode_t, dev_t);
#[fexit(function = "vfs_mknod")]
fn vfs_mknod(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_dentry(ctx, 2, 22) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// int vfs_mkobj(struct dentry *dentry, umode_t mode, int (*f)(struct dentry *, umode_t, void *), void *arg)
#[fexit(function = "vfs_mkobj")]
fn vfs_mkobj(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_dentry(ctx, 0, 23) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// int vfs_open(const struct path *, struct file *);
#[fexit(function = "vfs_open")]
fn vfs_open(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_file(ctx, 1, 24) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// ssize_t vfs_read(struct file *file, char __user *buf, size_t count, loff_t *pos)
#[fexit(function = "vfs_read")]
fn vfs_read(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_file(ctx, 0, 25) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// int vfs_rename(struct renamedata *rd)
#[fexit(function = "vfs_rename")]
fn vfs_rename(ctx: FExitContext) -> Result<(), i64>  {
    unsafe{
        let rndata: *const vmlinux::renamedata = ctx.arg(0);
        //let dent: *const vmlinux::dentry = bpf_probe_read_kernel(&(*entry).d_parent)?;
    
        let olddent: *const vmlinux::dentry = bpf_probe_read_kernel(&(*rndata).old_dentry)?;
        // let dent: *const vmlinux::dentry = bpf_probe_read_kernel(&(*(*rndata).new_dentry).d_parent)?;
    
        let val : i64 = match try_dentry(&ctx,olddent, 26){
            Ok(x) => x,
            Err(x) => x,
        };
    }
    Ok(())
}


// int vfs_rmdir(struct mnt_idmap *, struct inode *, struct dentry *);
#[fexit(function = "vfs_rmdir")]
fn vfs_rmdir(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_dentry(ctx, 2, 27) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// int vfs_truncate(const struct path *path, loff_t length)
#[fexit(function = "vfs_truncate")]
fn vfs_truncate(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_path(ctx, 0, 28) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// ssize_t vfs_write(struct file *file, const char __user *buf, size_t count, loff_t *pos)
#[fexit(function = "vfs_write")]
fn vfs_write(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_file(ctx, 0, 29) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

// static ssize_t vfs_writev(struct file *file, const struct iovec __user *vec, unsigned long vlen, loff_t *pos, rwf_t flags)
#[fexit(function = "vfs_writev")]
fn vfs_writev(ctx: FExitContext) -> i32 {
    match unsafe { fe_try_arg_file(ctx, 0, 30) } {
        Ok(_) => {},
        Err(_) => {},
    }
    0i32
}

fn fe_try_arg_file(ctx: FExitContext, path_arg_pos: u8,calltype: u8) -> Result<(), i64> {
    unsafe{
        let file: *const vmlinux::file = ctx.arg(path_arg_pos as usize);
        let path = bpf_probe_read_kernel(&(*file).f_path)?;
        let dent: *const vmlinux::dentry = path.dentry;
        
        let val : i64 = match try_dentry(&ctx, dent, calltype){
            Ok(x) => x,
            Err(x) => x,
        };
        Ok(())
    }
}

fn fe_try_arg_path(ctx: FExitContext, path_arg_pos: u8,calltype: u8) -> Result<(), i64> {
    unsafe{
        let path: *const vmlinux::path = ctx.arg(path_arg_pos as usize);
        let dent: *const vmlinux::dentry = bpf_probe_read_kernel(&(*path).dentry)?;
        
        let val : i64 = match try_dentry(&ctx, dent, calltype){
            Ok(x) => x,
            Err(x) => x,
        };
        Ok(())
    }
}

fn fe_try_arg_dentry(ctx: FExitContext, dent_arg_pos: u8,calltype: u8) -> Result<(), i64> {
    unsafe{
        let entry: *const vmlinux::dentry = ctx.arg(dent_arg_pos as usize);
        
        let val : i64 = match try_dentry(&ctx, entry, calltype){
            Ok(x) => x,
            Err(x) => x,
        };
        Ok(())
    }
}


// ========================================
// =          MAIN FUNCTIONS              =
// ========================================

fn try_dentry<T: EbpfContext>(ctx: &T, dent: *const vmlinux::dentry, call_type: u8) -> Result<i64, aya_ebpf::cty::c_long> {
    unsafe {
        let key: u32 = 0; // Assuming a single key for now
        let dir_inode = match INODEDATA.get(key) {
            Some(inode) => inode,
            None => return Err(2i64),
        };

        if (!in_dir(dent,*dir_inode)){
            // info!(ctx, "yeah");filename
            push_value_to_array(0, [0u8; 64], &BUF);
            return Ok(0i64);
        }

        let inode: *const vmlinux::inode = bpf_probe_read_kernel(&(*dent).d_inode)?;
        let inode_num: u64 = bpf_probe_read_kernel(&(*inode).i_ino)?;

        let len = pathToMap(dent,&BUF, 50, ctx, *dir_inode);
        let event_data = EventData {
            event_type: call_type,   // Example event type (you can change it based on your use case)
            len: len as u16, // Existing length value, cast to u16
            inod: inode_num,
            len2: 0u16,
            inod2: 0u64,
            rename_state: 0u8,
        };
        //EVENTS.output(ctx, &(len as u16), 0);
        EVENTS.output(ctx, &event_data, 0);
    };
    Ok(0i64)
}

unsafe fn in_dir(dent: *const vmlinux::dentry, dir_inode: u64) -> bool {
    unsafe{
        let mut current_dentry: *const vmlinux::dentry = dent;

        // Traverse up the directory structure by following parent dentries
        for i in 0..50 {  // Max depth of 50 to avoid infinite loops
            
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

    false // Return false if no match is found
}

//Just runs dnameToMap n times to get dnames up the directory
unsafe fn pathToMap<T: EbpfContext>(dent: *const vmlinux::dentry,array: &PerCpuArray<[u8; 64]>, depth: u8, ctx: &T, dir_inode: u64) -> u32{
    let mut fullLength = 1;
    let mut arrayOffset = 1;

    let mut slash: [u8; 64] = [0; 64]; // Initialize all elements to 0
    slash[0] = 47; // Set the first element to 47
    
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

        if u64::from(inode_num) == dir_inode {
            break;
        }

        //Updates map then offsets position in map by "len"
        let len = dnameToMap(current_dentry,&array,arrayOffset, ctx);
        arrayOffset += 1;
        fullLength += len;

        //Add slash (47)
        push_value_to_array(arrayOffset, slash, &array);
        arrayOffset += 1;
        fullLength += 1;
        

        current_dentry = match bpf_probe_read_kernel(&(*current_dentry).d_parent) {
            Ok(parent) => parent,
            Err(_) => break, // If reading parent fails, stop traversal
        };
    }

    return fullLength - 1 + (fullLength / 255u32);
}

//fills BUFFER at arrayOffset with characters in d_name.name
unsafe fn dnameToMap<T: EbpfContext>(dent: *const vmlinux::dentry,array: &PerCpuArray<[u8; 64]>, arrayOffset: u32, ctx : &T) -> u32 {
    
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

            let name: [u8; 64] = match bpf_probe_read_kernel((((qstring).name).add(offset as usize) as *const [u8; 64])) {
                Ok(n) => n,
                Err(_) => return 0,
            };
            
            if msgLen < (n + 1) * 64{
                end = true;
            }

            push_value_to_array(n + arrayOffset, name, &array);
        }
        if end{
            return n * 64 + msgLen % 64;

            break;
        }
    }

    return 0u32
}

unsafe fn push_value_to_array<T:Copy>(index: u32, value: T, array : &PerCpuArray<T>){
    let output_data_ptr: *mut aya_ebpf::maps::PerCpuArray<T> = array as *const _ as *mut aya_ebpf::maps::PerCpuArray<T>;
    // let output_data_ptr: *mut aya_ebpf::maps::PerCpuArray<T> = array.get_ptr_mut()?;
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
