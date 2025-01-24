use aya::maps::{PerCpuArray, PerfEventArray};
use aya::programs::KProbe;
use aya::util::online_cpus;
use aya::Ebpf;
use aya::{
    include_bytes_aligned,
    Bpf,
    maps::{HashMap,Array},
};
use client::send_full_contents_of_file;
use env_logger::filter;
use filehasher::hash_check;
use std::sync::Arc;
use aya_log::BpfLogger;
use libc::SIGINT;
use log::{info, warn, debug};
use tokio::signal::unix::{signal,SignalKind};
use tokio::signal;
use std::fs;
use std::ops::Index;
use std::os::linux::raw;
use std::process;
use std::io::BufReader;
use serde_json::{self, Value};
use aya::maps::AsyncPerfEventArray;
use std::{error::Error, thread};
use signal_hook::{consts::SIGUSR1, iterator::Signals};
use bytes::{Buf, BytesMut};

use tokio::task; // or async_std::task
mod client;
pub mod filehasher;
pub mod fileFilter;
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    
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
        "../../target/bpfel-unknown-none/debug/ullfs"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut bpf: Ebpf = Ebpf::load(include_bytes_aligned!(
        "../../target/bpfel-unknown-none/release/ullfs"
    ))?;
    // if let Err(e) = BpfLogger::init(&mut bpf) {
    //     // This can happen if you remove all log statements from your eBPF program.
    //     warn!("failed to initialize eBPF logger: {}", e);
    // }
    let program: &mut KProbe = bpf.program_mut("vfs_write").unwrap().try_into()?;
    program.load()?;
    program.attach("vfs_write", 0)?;

    //*Lets read the config file */
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
    // Debugging data
    // println!("Conf file output: {}", &watch_dir);
    // Get the metadata from the watch_dir
    let w_dir = match fs::metadata(watch_dir){
        Ok(x) => x,
        Err(e) => {
            panic!("Error: Directory {} not found, something must be wrong with your config file\n{}", &watch_dir, e);
        }
    };
    
    
    
    // Get the inode from the metadata
    let block_addr: u64 = std::os::linux::fs::MetadataExt::st_ino(&w_dir);
    println!("Block Address: {}", block_addr);
    {
        // Initialize the inode map
        let mut inodesdata: Array<_, u64> = Array::try_from(bpf.take_map("INODEDATA").unwrap())?;
        inodesdata.set(0, block_addr, 0)?;
    }
    {
        // Initialize the program data map
        // ID 0: PID for this program
        let mut progdata: Array<_, u64> = Array::try_from(bpf.take_map("PROGDATA").unwrap())?;
        let progid = process::id();
        let progid_64 : u64 = u64::from(progid);
        progdata.set(0, progid_64, 0)?
    }
    
    {
        // l
        // tokio::time::sleep
        let mut perf_array = AsyncPerfEventArray::try_from(bpf.take_map("EVENTS").unwrap())?;
        let buf_array: PerCpuArray<_,u8> = match PerCpuArray::try_from(bpf.take_map("BUF").unwrap()){
            Ok(x) => x,
            Err(_) => {
                panic!("PerCpuArray failed to initialize");
            }
        };
        let s_buf = Arc::new(buf_array);
        
        for cpu_id in online_cpus().map_err(|(_, error)| error)? {
            // open a separate perf buffer for each cpu
            let mut buf = perf_array.open(cpu_id, None)?;
            // let val_buf = buf_array.get(&1 , 0)?;
            // let bufData: Array<_, u8> = match Array::try_from(bpf.take_map("BUF").unwrap()){
            //     Ok(x) => x,
            //     Err(_) => {
            //         panic!("Error: Bufdata not set up properly");
            //     }
            // };
            // let val = buf_array.get(&0, 0);
            // process each perf buffer in a separate task
            let s_buf_clone = Arc::clone(&s_buf);

            task::spawn(async move {
                let filter: &fileFilter::Filter = fileFilter::Filter::get_instance();
                
                // let mut bufArray: PerCpuArray<_, u8> = match PerCpuArray::try_from(bpf.map_mut("BUF").unwrap()){
                //     Ok(x) => x,
                //     Err(_) => {
                //         panic!("PerCPUArray failed to set up");
                //     }
                // };

                let mut buffers = (0..4)
                    .map(|_| BytesMut::with_capacity(4))
                    .collect::<Vec<_>>();
                
                loop {
                    
                    // wait for events
                    let event = buf.read_events(&mut buffers).await.unwrap(); /*{
                        Some(x) => x,
                        None => {
                            panic!("Buffer read events bad somehow");
                        }
                    };*/
                    
                    for i in 0..event.read {
                        let buf = &mut buffers[i];
                        let len = buf.get_u8();
                        let len2 = buf.get_u8();
                        // We have the 2 u8's that consist of a u16
                        // Even though get_u16() existed, it gave me really large numbers
                        // So I instead combined two u8's
                        // We know the max size is 4096 so we can use values higher than that as error codes
                        // Or 0
                        let totalLen: u16 = (len2 as u16 * 255u16) + len as u16;
                        // println!("Event received {}: {}, {}", totalLen, len, len2);
                        let cpus = match online_cpus().map_err(|(_, error)| error){
                            Ok(x) => x,
                            Err(_) => {
                                panic!("Error getting online cpus");
                            }
                        };

                        let mut filename = String::new();
                        for i in 1..totalLen{
                            let val : u8 = match s_buf_clone.get(&(i as u32), 0){
                                Ok(x) => {
                                    match x.get(cpu_id as usize){
                                        Some(y) => *y,
                                        None => 0,
                                    }
                                },
                                Err(_) => 0,
                            };
                        
                            if val == 0{
                                filename.push(166 as char); // ¦ for empty spaces for debugging
                            }
                            else{
        
                                filename.push(val as char); // Convert u8 to char and push to String
                            }
                        }
                        filename.push('/'); 
                        println!("{}",filename);
                        // Correct reversed path
                        let corrected_path: String = filename
                            .split('/')
                            .rev()
                            .collect::<Vec<&str>>()
                            .join("/");
                        println!("Unreversed: {}", corrected_path);


                        // Now we actually get to deal with deltas
                        let final_path = corrected_path.as_str();
                        let shouldFilter = filter.should_filter(final_path);

                        // Extract deltas
                        if(!shouldFilter){
                            send_full_contents_of_file(final_path);
                        }
                    }
                }
                // Ok::<_, PerfBufferError>(())
            });
        }
    }
    //{Index, Value, Flags}
    let t = tokio::signal::ctrl_c().await;
    println!("Exiting");
    Ok(())
}
fn coerce_static<'a, T>(v: &'a T) -> &'a T {
    &v
}