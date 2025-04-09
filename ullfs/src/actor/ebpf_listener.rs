use args::Args;
use aya::maps::{array, PerCpuArray, PerfEventArray};
use aya::Btf;
use aya::programs::{FEntry, FExit, KProbe, Lsm};
use aya::util::online_cpus;
use aya::Ebpf;
use aya::{
    include_bytes_aligned,
    maps::Array,
};
//use client::send_full_contents_of_file_tcp;
use env_logger::filter;
// use filehasher::hash_check;
use steady_state::*;
use tokio::fs::ReadDir;
use tokio::sync::mpsc::{self, Sender};
use std::error::Error;
use std::sync::Arc;
use log::debug;
use std::{clone, fs};
use std::process;
use std::io::BufReader;
use serde_json::{self, Value};
use aya::maps::AsyncPerfEventArray;
use bytes::{Buf, BytesMut};
use anyhow::Result;

use tokio::task; // or async_std::task
// use tokio::time::Sleep;
// mod client;
// pub mod filehasher;
// pub mod fileFilter;
// pub mod createPacket;
// pub mod fileDifs;
// pub mod hashFileDif;
// pub mod client_tcp;

use std::path::Path;

use std::os::unix::fs::MetadataExt; // For .ino() method on metadata.
use std::path::PathBuf;
use tokio::io;

use crate::fileFilter;
const BATCH_SIZE: usize = 7000;

#[derive(Copy, Clone)]
pub(crate) struct RuntimeState {
    value: u64,
    buffer: [u8; BATCH_SIZE], // Use a byte buffer for TCP streams
}

impl RuntimeState {
    pub(crate) fn new(value: i32) -> Self {
        RuntimeState {
            value: value as u64,
            buffer: [0; BATCH_SIZE], // Initialize the byte buffer
        }
    }
}

fn attach_program(bpf: &mut Ebpf, prog_name: &str) -> Result<()> {
    let btf = Btf::from_sys_fs()?;
    // Retrieve the program and convert it to an Lsm type.
    let program: &mut Lsm = bpf
        .program_mut(prog_name)
        .unwrap()
        .try_into()?;
    // Load and attach the program.
    program.load(prog_name, &btf)?;
    program.attach()?;
    Ok(())
}

fn attach_program_fexit(bpf: &mut Ebpf, prog_name: &str) -> Result<()> {
    let btf = Btf::from_sys_fs()?;
    // Retrieve the program and convert it to an Lsm type.
    let program: &mut FExit = bpf
        .program_mut(prog_name)
        .unwrap()
        .try_into()?;
    // Load and attach the program.
    program.load(prog_name, &btf)?;
    program.attach()?;
    Ok(())
}

type BufferType = PerCpuArray<aya::maps::MapData, [u8; 64]>;
#[derive(Clone, Copy)]
pub enum ChangeType {
    write,
    create_file,
    create_dir,
    delete,
    move_fdir,
    

}
pub struct TcpData {
    pub filename: String,
    pub old_filename: String,
    pub change_type: ChangeType
}
async fn extract_filename(
    total_len: usize,
    s_buf_clone: Arc<BufferType>,
    cpu_id: usize,
) -> String {
    let mut filename = String::new();
    let mut array_index = 1;
    let mut iteration = 1;

    while iteration <= total_len {
        // Attempt to get the 64-byte array from the buffer.
        let val: [u8; 64] = match s_buf_clone.get(&(array_index as u32), 0) {
            Ok(x) => match x.get(cpu_id) {
                Some(y) => *y,
                None => [0; 64],
            },
            Err(_) => [0; 64],
        };
        array_index += 1;

        if val == [0; 64] {
            // Use the debug symbol (¦) for empty spaces.
            filename.push(166 as char);
            iteration += 1;
        } else {
            // Process non-null bytes until a null byte is encountered.
            for &byte in val.iter().take_while(|&&c| c != 0) {
                filename.push(byte as char);
                iteration += 1;
            }
        }
    }

    // Reverse the path components.
    let corrected_path = filename
        .split('/')
        .rev()
        .collect::<Vec<&str>>()
        .join("/");

    println!("\tUnreversed: {}", corrected_path);
    corrected_path
}

pub async fn ullfs_write(filepath: String, tx: &mut Sender<TcpData>){
    let data: TcpData = TcpData { filename: filepath.to_string(), old_filename: String::new(), change_type: ChangeType::write};
    
    println!("File written: {}", filepath);
    tx.send(data).await;
}

pub async fn ullfs_create_dir(filepath: String, tx: &mut Sender<TcpData>){
    let data: TcpData = TcpData { filename: filepath.to_string(), old_filename: String::new(), change_type: ChangeType::create_dir };
    println!("Directory created: {}", filepath);
    tx.send(data).await;
}

pub async fn ullfs_create_file(filepath: String, tx: &mut Sender<TcpData>){
    let data: TcpData = TcpData { filename: filepath.to_string(), old_filename: String::new(), change_type: ChangeType::create_file };

    println!("File created: {}", filepath);
    tx.send(data).await;
}

pub async fn ullfs_delete(filepath: String, tx: &mut Sender<TcpData>){
    let data: TcpData = TcpData { filename: filepath.to_string(), old_filename: String::new(), change_type: ChangeType::delete };

    println!("File|Directory {} deleted", filepath);
    tx.send(data).await;
}

pub async fn ullfs_rename(filepath_from: String, filepath_to: String, tx: &mut Sender<TcpData>){
    let data: TcpData = TcpData { filename: filepath_from.to_string(), old_filename: filepath_to.to_string(), change_type: ChangeType::move_fdir };

    println!("File renamed from {} to {}", filepath_from, filepath_to);
    tx.send(data).await;
}

pub async fn ullfs_move(filepath_from: String, filepath_to: String, tx: &mut Sender<TcpData>){
    let data: TcpData = TcpData { filename: filepath_from.to_string(), old_filename: filepath_to.to_string(), change_type: ChangeType::move_fdir };

    println!("File|Directory Moved {} to {}", filepath_from, filepath_to);
    tx.send(data).await;
}

pub async fn ullfs_move_into_watch(filepath: String, tx: &mut Sender<TcpData>){
    let data: TcpData = TcpData { filename: filepath.to_string(), old_filename: String::new(), change_type: ChangeType::write };

    println!("File|Directory Moved into watch directory [Send all] {}", filepath);
    tx.send(data).await;
}

pub async fn run(context: SteadyContext
    ,transmitter: SteadyTx<TcpData>
    ,state: SteadyState<RuntimeState>
) -> Result<(),Box<dyn Error>> {
    // Call internal_behavior from here and do some initial setup:

    // if needed CLI Args can be pulled into state from _cli_args
    let _cli_args = context.args::<Args>();
    println!("Ebpf listener active");
    // monitor consumes context and ensures all the traffic on the chosen channels is monitored
    // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
    let mut cmd = into_monitor!(context, [], [transmitter]);
    // while cmd.is_running(|| {
    //     true
    // })
    //  {

    //  }
    // // loop {}
    // Ok(())
    // let cmd = into_monitor!(context, [], [transmitter]);
    internal_behavior(cmd, transmitter, state).await

    // Ok(())
}
// #[tokio::main]
async fn internal_behavior<C: SteadyCommander>(
    mut cmd: C, 
    transmitter: SteadyTx<TcpData>,
    state: SteadyState<RuntimeState>,
) -> Result<(),Box<dyn Error>> {
    let mut state_guard = steady_state(&state, || RuntimeState::new(1)).await;
    env_logger::init();

    let (mut bpf, block_addr, watch_dir_string) = setup_ebpf().await?;
    if let Some(mut _state) = state_guard.as_mut()
    {
        let mut perf_array = AsyncPerfEventArray::try_from(bpf.take_map("EVENTS").unwrap())?;
        let buf_array: PerCpuArray<_,[u8;64]> = match PerCpuArray::try_from(bpf.take_map("BUF").unwrap()){
            Ok(x) => x,
            Err(_) => {
                panic!("PerCpuArray failed to initialize");
            }
        };

        let second_buf_array: PerCpuArray<_,[u8;64]> = match PerCpuArray::try_from(bpf.take_map("BUFTWO").unwrap()){
            Ok(x) => x,
            Err(_) => {
                panic!("PerCpuArray failed to initialize");
            }
        };

        let s_buf = Arc::new(buf_array);
        let second_s_buf = Arc::new(second_buf_array);

        let arc_w_dir = Arc::new(watch_dir_string);
        let (tx_orig, mut rx) = mpsc::channel(100);

        for cpu_id in online_cpus().map_err(|(_, error)| error)? {
            // open a separate perf buffer for each cpu
            let mut buf = perf_array.open(cpu_id, None)?;

            let s_buf_clone: Arc<PerCpuArray<aya::maps::MapData, [u8; 64]>> = Arc::clone(&s_buf);
            let second_s_buf_clone: Arc<PerCpuArray<aya::maps::MapData, [u8; 64]>> = Arc::clone(&second_s_buf);

            let arc_w_dir_clone = Arc::clone(&arc_w_dir);
            let mut tx = tx_orig.clone();
            task::spawn(async move {
                let filter: &fileFilter::Filter = fileFilter::Filter::get_instance();

                let mut buffers = (0..4)
                    .map(|_| BytesMut::with_capacity(4))
                    .collect::<Vec<_>>();

                loop {
                    let DEBUG = true;
                    if(DEBUG){
                        println!("---EVENTS---");
                    }
                    // wait for events
                    let event = buf.read_events(&mut buffers).await.unwrap();
                        
                    let mut inode_num: u64 = block_addr;
                    let mut second_inode_num: u64 = block_addr;
                    let mut rename_data: u8 = 0;
                    let mut check_inode: bool = false;
                    let mut is_dir: bool = false;
                    let mut temp_path: String = String::new();
                    let mut base_path: String = String::new();
                    let mut return_path: String = String::new();

                    //0 is default
                    //1 is rename
                    //2 is d_instantiate file created
                    //3 is d_instattiate dir created
                    //4 is rmdir
                    //5 is move
                    //6 is move in from outside watchdir
                    let mut mode: u8 = 0;
                    let mut arg1: String = String::new();
                    let mut arg2: String = String::new();
                    for i in 0..event.read {
                        let buf = &mut buffers[i];

                        let temp_inode_num: u64 = buf.get_u64_le();
                        let second_temp_inode_num: u64 = buf.get_u64_le();
                        let total_len: u16 = buf.get_u16_le();
                        let second_total_len: u16 = buf.get_u16_le();
                        let data: u8 = buf.get_u8();
                        let rnd: u8 = buf.get_u8();

                        rename_data = rnd;

                        // println!("temp_inode_num {}", temp_inode_num);
                        // println!("second_temp_inode_num {}", second_temp_inode_num);
                        // println!("second_total_len {}", second_total_len);
                        // println!("total_len {}", total_len);
                        // println!("data {}", data);
                        // println!("rename data {}", rename_data);

                        if block_addr != temp_inode_num{
                            inode_num = temp_inode_num;
                            second_inode_num = second_temp_inode_num;
                        }

                        if(DEBUG){
                            print!("{} ",inode_num);
                        }

                        if(DEBUG){
                            match data {
                                // Primary programs
                                0  => println!("path_unlink"),
                                1  => println!("path_mkdir"),
                                2  => println!("path_rmdir"),
                                3  => println!("path_mknod"),
                                4  => println!("path_symlink"),
                                5  => println!("path_link"),
                                6  => println!("path_rename"),
                                7  => println!("path_notify"),
                                8  => println!("path_chmod"),
                                9  => println!("path_chown"),
                                10 => println!("path_chroot"),
                        
                                11 => println!("inode_create"),
                                12 => println!("inode_link"),
                                13 => println!("inode_unlink"),
                                14 => println!("inode_syslink"),
                                15 => println!("inode_mkdir"),
                                16 => println!("inode_rmdir"),
                                17 => println!("inode_mknod"),
                                18 => println!("inode_rename"),
                                
                                19 => println!("d_instantiate"),
                        
                                // fexit programs
                                20 => println!("vfs_iter_write"),
                                21 => println!("vfs_mkdir"),
                                22 => println!("vfs_mknod"),
                                23 => println!("vfs_mkobj"),
                        
                                24 => println!("vfs_open"),
                                25 => println!("vfs_read"),
                                26 => println!("vfs_rename"),
                                27 => println!("vfs_rmdir"),
                                28 => println!("vfs_truncate"),
                                29 => println!("vfs_write"),
                                30 => println!("vfs_writev"),
                        
                                // Fallback for invalid indices.
                                _  => println!("Invalid function index: {}", data),
                            }
                        }

                        // ========================================
                        // =           DECODE SYSCALLS            =
                        // ========================================

                        let corrected_path: String = extract_filename(total_len as usize, s_buf_clone.clone(), cpu_id as usize).await;

                        //inode_rename
                        if(data == 18){
                            let second_corrected_path: String = extract_filename(second_total_len as usize, second_s_buf_clone.clone(), cpu_id as usize).await;

                            println!("{}", corrected_path);
                            println!("{}", second_corrected_path);

                            if(rename_data == 1){
    
                                // Split the path into components, ignoring empty parts.
                                let parts: Vec<&str> = corrected_path
                                    .split('/')
                                    .filter(|s| !s.is_empty())
                                    .collect();
    
                                // If there is more than one component, remove the last component and rebuild.
                                // Otherwise, return an empty string.
                                let parent = if parts.len() > 1 {
                                    // Join all components except the last and prepend a '/'
                                    format!("/{}", parts[..parts.len()-1].join("/"))
                                } else {
                                    "".to_string()
                                };
                                
                                if(parent != second_corrected_path){
                                    //Move Action
                                    check_inode = true;
                                    mode = 5;
                                    temp_path = corrected_path.clone();

                                    base_path = second_corrected_path;
                                }

                                else{
                                    //Default
                                    check_inode = true;
                                    mode = 1;
                                    temp_path = corrected_path.clone();

                                    base_path = parent;
                                }
    
                                // println!("Original: {} -> Parent: {}", corrected_path, parent);
                            }
                            else if (rename_data == 2) {
                                // println!("Moved In {}", second_corrected_path);
                                check_inode = true;
                                mode = 6;
                                // inode_num = second_inode_num;
                                temp_path = corrected_path.clone();

                                base_path = second_corrected_path;
                                //Moved out of watch dir [Delete]
                            }
                            else if (rename_data == 3) {
                                //println!("Moved Out [Deleted] {}", corrected_path);
                                // println!("File|Directory {} Deleted", corrected_path);
                                ullfs_delete(corrected_path.clone(), &mut tx).await;
                                return_path = corrected_path.clone();
                                //Moved into watch dir [Send file/all subfiles]
                            }
                            else{
                                println!("IDK");
                            }
                        }

                        //path_mkdir or inode_mkdir
                        if(data == 1 || data == 15){
                            is_dir = true;
                            base_path = corrected_path.clone();
                        }
                        //path_mknod or inode_create
                        if(data == 3 || data == 11){
                            base_path = corrected_path.clone();
                        }
                        //d_instantiate
                        if(data == 19){
                            if(is_dir){
                                mode = 3;
                            }
                            else{
                                mode = 2;
                            }
                            check_inode = true;
                            temp_path = corrected_path.clone();
                        }

                        //vfs_write
                        if(data == 29){
                            // println!("Write at {}", corrected_path);
                            ullfs_write(corrected_path.clone(), &mut tx).await;
                            return_path = corrected_path.clone();
                        }

                        //path_rmdir or inode_rmdir
                        if(data == 2 || data == 16){
                            check_inode = true;
                            mode = 4;
                            temp_path = corrected_path.clone();
                        }
                    }

                    // ========================================
                    // =             CHECK INODE              =
                    // ========================================

                    if(check_inode){
                        if(block_addr == inode_num){
                            continue;
                        }
    
                        let dir_str: &str = &arc_w_dir_clone;
                        if(base_path.is_empty()){
                            base_path = dir_str.to_string();
                        }
                        else{
                            base_path = dir_str.to_string() + &base_path;
                        }
                        let mut path: Option<PathBuf> = None;
                        
                        // Try to find the file with the matching inode
                        if let Ok(mut entries) = tokio::fs::read_dir(base_path.clone()).await {
                            while let Ok(Some(entry)) = entries.next_entry().await {
                                if let Ok(metadata) = entry.metadata().await {
                                    if metadata.ino() == inode_num {
                                        path = Some(entry.path());
                                        break;
                                    }
                                }
                            }
                        }
                        
                        // Process the found path (or handle case where path wasn't found)
                        match path {
                            Some(file_path) => {
                                let mut relative_path: String = match file_path.strip_prefix(dir_str) {
                                    Ok(path) => path.to_string_lossy().into_owned(),
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                        return; // Exit the function if there's an error
                                    }
                                };

                                if !relative_path.starts_with('/') {
                                    relative_path.insert(0, '/');
                                }

                                // let t_p: String = temp_path.replace("¦", "");
                                // relative_path = relative_path.replace("¦", "");

                                // return_path = relative_path.clone();
                                match mode {
                                    1 => {
                                        // println!("File {} renamed to {}", temp_path, relative_path);
                                        ullfs_rename(temp_path, relative_path, &mut tx).await;
                                    },
                                    2 => {
                                        // println!("File {} created", relative_path);
                                        ullfs_create_file(relative_path, &mut tx).await;
                                    },
                                    3 => {
                                        // println!("Directory {} created", relative_path);
                                        ullfs_create_dir(relative_path, &mut tx).await;
                                    },
                                    4 => println!("errr"),
                                    5 => {
                                        // println!("File|Directory Moved {} to {}", temp_path, relative_path);
                                        ullfs_move(temp_path, relative_path, &mut tx).await;
                                    },
                                    6 => {
                                        // println!("File|Directory Moved into watch directory [Send all] {}", relative_path);
                                        ullfs_move_into_watch(relative_path, &mut tx).await;
                                    },
                                    _ => (),
                                }

                                // match mode {
                                //     1 | 5 => {
                                //         arg1 = temp_path.clone(); 
                                //         arg2 = relative_path.clone();
                                //     },
                                //     2 | 3 | 6 => arg1 = relative_path.clone(),
                                //     _ => (),
                                // }
                                // Do something with the found path
                                // println!("Found file with inode {}: {:?}", inode_num, file_path);
                            },
                            None => {
                                // println!("DEBUG: return_path has been set to: '{}'", return_path);
                                // return_path = temp_path.clone();

                                // temp_path = temp_path.replace("¦", "");
                                // base_path = base_path.replace("¦", "");

                                match mode {
                                    1 => {
                                        // println!("File|Directory {} deleted", temp_path);
                                        ullfs_delete(temp_path, &mut tx).await;
                                    },
                                    2 => println!("Something Weird Happened {} bp {}",temp_path, base_path),
                                    3 => println!("Something Weird Happened [dir]{} bp {}", temp_path, base_path),
                                    4 => {
                                        // println!("Directory {} deleted [rmdir]", temp_path);
                                        ullfs_delete(temp_path, &mut tx).await;
                                    },
                                    5 => println!("Something"),
                                    6 => println!("Something else"),
                                    _ => (),
                                }
                                // match mode {
                                //     // 2 | 3 => {
                                //     //     arg1 = temp_path.clone(); 
                                //     //     arg2 = base_path.clone();
                                //     // },
                                //     1 | 4 => {
                                //         arg1 = temp_path.clone();
                                //         return_path = temp_path.clone();
                                //     },
                                //     _ => (),
                                // }
                                // Handle case where no file with matching inode was found
                                // println!("No file found with inode {}", inode_num);
                            }
                        }
                    }

                    // ========================================
                    // =       EXTRACT DELTA AND SEND         =
                    // ========================================

                    // Now we actually get to deal with deltas
                    // Create the final path from the path we got and the watch directory

                    // This will have to be updated most likely using one of these
                    // arg1
                    // arg2
                    // return_path


                    // if return_path.starts_with('/') {
                    //     return_path.remove(0);
                    // }
                    // let final_path = String::from(filter.get_base_dir()) + return_path.as_str();
                    // let should_filter = filter.should_filter(final_path.as_str());

                    // if(DEBUG){
                    //     println!("FINALPATH {}", final_path);
                    //     println!("RETURNPATH {}", return_path);
                    //     println!("Arg1 {}", return_path);
                    // }
                    

                    // // Extract deltas
                    // if (!should_filter) {
                    //     // send_full_contents_of_file_tcp(final_path.as_str());
                    //     //client_tcp::write_full_file_to_connections(final_path.as_str());
                    // }

                    // //Events
                }
                // Ok::<_, PerfBufferError>(())
            });
        }
        //{Index, Value, Flags}
        let mut received_data = rx.recv().await;

        let mut transmit_lock = transmitter.lock().await;

        while cmd.is_running(&mut || transmit_lock.mark_closed()){
            // This begins the weird translation between tokio and steady state
            // It exists purely because Steady State does not work with Aya alone
            // We needed multi transmitters and a single receiver.
            // All those transmitters send there data here, which will then send off to other actors
            // println!("{}", received_string);
            let mut received_tcp_data = match received_data {
                Some(x) => x,
                None => {
                    println!("Received none, end of receivers"); // Hopefully this code works
                    break;
                }
            };
            println!("Data Recieved: {}-{}", received_tcp_data.change_type as u8, received_tcp_data.filename);
            match cmd.send_async(&mut transmit_lock, received_tcp_data, SendSaturation::IgnoreAndWait).await {
                Ok(_) => {
                    println!("Sent data");
                },
                Err(x) => {
                    println!("Error on send_async: {}-{}", x.change_type as u8, x.filename);
                }
            };
            cmd.relay_stats();
            received_data = rx.recv().await;

        }
    }
    println!("Finished!");

    // loop{}

    Ok(())
    // let t = tokio::signal::ctrl_c().await;
    // println!("Exiting");
    // Ok(())
}

pub async fn setup_ebpf() -> Result<(Ebpf, u64, String), Box<dyn Error>>{
    // ========================================
    // =              INIT EBPF               =
    // ========================================
    
    // Bump the memlock rlimit. This is needed for older kernels that don't use the
    // new memcg based accounting, see https://lwn.net/Articles/837122/
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };

    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("remove limit on locked memory failed, ret is: {}", ret);
    }

    // This will include your eBPF object file as raw bytes at compile-time and load it at
    // runtime. This approach is recommended for most real-world use cases. If you would
    // like to specify the eBPF program at runtime rather than at compile-time, you can
    // reach for `Bpf::load_file` instead.
    #[cfg(debug_assertions)]
    let mut bpf: Ebpf = Ebpf::load(include_bytes_aligned!(
        "../../../target/bpfel-unknown-none/debug/ullfs"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut bpf: Ebpf = Ebpf::load(include_bytes_aligned!(
        "../../../target/bpfel-unknown-none/release/ullfs"
    ))?;

    // ========================================
    // =          ATTACH EBPF PROGRAMS        =
    // ========================================
    let primary_programs = [
        "path_unlink",
        "path_mkdir",
        "path_rmdir",
        "path_mknod",
        "path_symlink",
        "path_link",
        "path_rename",
        "path_notify",
        "path_chmod",
        "path_chown",
        "path_chroot",

        "inode_create",
        "inode_link",
        "inode_unlink",
        "inode_symlink",
        "inode_mkdir",
        "inode_rmdir",
        "inode_mknod",
        "inode_rename",
        
        "d_instantiate",
    ];

    let fexit_programs = [
        // "vfs_iter_write",
        // "vfs_mkdir",
        // "vfs_mknod",
        // "vfs_mkobj",

        // "vfs_open",
        // "vfs_read",
        // "vfs_rename",
        // "vfs_rmdir",
        // "vfs_truncate",
        "vfs_write",
        // "vfs_writev",
    ];

    // Attach each program.
    for prog in &primary_programs {
        attach_program(&mut bpf, prog)?;
    }

    for prog in &fexit_programs {
        attach_program_fexit(&mut bpf, prog)?;
    }

    // ========================================
    // =              INIT EBPF               =
    // ========================================

    let conf_file : fs::File = match fs::File::open("./config.json"){
        Ok(x) => x,
        Err(e) => {
            panic!("Error: config.json missing or destroyed.\n{}", e)
        }
    };

    // Convert to buffer for serde_json
    let reader = BufReader::new(conf_file);
    let conf : Value = match serde_json::from_reader(reader){
        Ok(x) => x,
        Err(e) => {
            panic!("Error: config.json structure damaged.\n{}", e);
        }
    }; 

    // Read from the json structure (Basically acts as a hashmap at this point)
    let watch_dir : &str = match &conf["watch_dir"].as_str() {
        None => {
            panic!("Error: watch_dir was not a string in config.json");
        }
        Some(x) => x,
    };
    let watch_dir_string : String = String::from(watch_dir);

    // Get the metadata from the watch_dirvfs_mkdir
    let w_dir = match fs::metadata(watch_dir){
        Ok(x) => x,
        Err(e) => {
            panic!("Error: Directory {} not found, something must be wrong with your config file\n{}", &watch_dir, e);
        }
    };

    // Get the inode from the metadata
    let block_addr: u64 = std::os::linux::fs::MetadataExt::st_ino(&w_dir);
    println!("Block Address: {}", block_addr);


    // ========================================
    // =            INIT EBPF MAPS            =
    // ========================================

    {
        let mut inodesdata: Array<_, u64> = Array::try_from(bpf.take_map("INODEDATA").unwrap())?;
        inodesdata.set(0, block_addr, 0)?;
    }

    {
        // ID 0: PID for this program
        let mut progdata: Array<_, u64> = Array::try_from(bpf.take_map("PROGDATA").unwrap())?;
        let progid = process::id();
        let progid_64 : u64 = u64::from(progid);
        progdata.set(0, progid_64, 0)?
    }
    

    return Ok((bpf, block_addr, watch_dir_string));
}