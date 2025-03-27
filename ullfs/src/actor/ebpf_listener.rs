use aya::maps::{array, PerCpuArray, PerfEventArray};
use aya::programs::KProbe;
use aya::util::online_cpus;
use aya::Ebpf;
use aya::{
    include_bytes_aligned,
    maps::Array,
};
//use client::send_full_contents_of_file_tcp;
use env_logger::filter;
use steady_state::{SteadyContext, SteadyRx, SteadyState, SteadyTx};
use tokio::net::TcpStream;
use std::error::Error;
use std::sync::Arc;
use log::debug;
use std::{fs, future};
use std::process;
use std::io::BufReader;
use serde_json::{self, Value};
use aya::maps::AsyncPerfEventArray;
use bytes::{Buf, BytesMut};
use crate::{client_tcp, fileFilter}
use tokio::task; // or async_std::task
// use tokio::time::Sleep;

pub async fn run(context: SteadyContext
    ,tcp_msg_rx: SteadyRx<TcpStream>
    ,tcp_conn_tx: SteadyTx<TcpStream>, state: SteadyState<RuntimeState>
) -> Result<(),Box<dyn Error>> {
    // let dif = fileDifs::FileData::get_instance();
    // let old = dif.get_file_delta("/home/zmanjaroschool/TestDir/testDif.txt");
    // let t = tokio::signal::ctrl_c().await;
    // let new = dif.get_file_delta("/home/zmanjaroschool/TestDir/testDif.txt");
    // println!("Old size: {}. New size: {}", old.1.len(), new.1.len());
    // println!("New start: {}. New end: {}", new.0, new.2);
    // return Ok(());
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
        "../../../target/bpfel-unknown-none/debug/ullfs"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut bpf: Ebpf = Ebpf::load(include_bytes_aligned!(
        "../../../target/bpfel-unknown-none/release/ullfs"
    ))?;
    // if let Err(e) = BpfLogger::init(&mut bpf) {
    //     // This can happen if you remove all log statements from your eBPF program.
    //     warn!("failed to initialize eBPF logger: {}", e);
    // }

    {
        let program: &mut KProbe = bpf.program_mut("vfs_write").unwrap().try_into()?;
        program.load()?;
        program.attach("vfs_write", 0)?;
    }

    {
        let program: &mut KProbe = bpf.program_mut("vfs_mkdir").unwrap().try_into()?;
        program.load()?;
        program.attach("vfs_mkdir", 0)?;
    }

    {
        let program: &mut KProbe = bpf.program_mut("vfs_rmdir").unwrap().try_into()?;
        program.load()?;
        program.attach("vfs_rmdir", 0)?;
    }

    {
        let program: &mut KProbe = bpf.program_mut("vfs_rename").unwrap().try_into()?;
        program.load()?;
        program.attach("vfs_rename", 0)?;
    }

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
    let watch_dir_string : String = String::from(watch_dir);
    // Debugging data
    // println!("Conf file output: {}", &watch_dir);
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
        let buf_array: PerCpuArray<_,[u8;64]> = match PerCpuArray::try_from(bpf.take_map("BUF").unwrap()){
            Ok(x) => x,
            Err(_) => {
                panic!("PerCpuArray failed to initialize");
            }
        };
        let s_buf = Arc::new(buf_array);
        // let watch_dir_clone = Arc::new(watch_dir);
        for cpu_id in online_cpus().map_err(|(_, error)| error)? {
            // open a separate perf buffer for each cpu
            let mut buf = perf_array.open(cpu_id, None)?;

            let s_buf_clone = Arc::clone(&s_buf);
            
            task::spawn(async move {
                let filter: &fileFilter::Filter = fileFilter::Filter::get_instance();

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
                        let data = buf.get_u8();

                        println!("{}",data);
                        

                        match data {
                            0 => {
                                // VFS_Write
                                let total_len: u16 = (len2 as u16 * 255u16) + len as u16;
                                // println!("Event received {}: {}, {}", totalLen, len, len2);
                                // let cpus = match online_cpus().map_err(|(_, error)| error){
                                //     Ok(x) => x,
                                //     Err(_) => {
                                //         panic!("Error getting online cpus");
                                //     }
                                // };
        
                                let mut filename = String::new();
        
                                let mut array_index  = 1;
                                println!("totalLen: {}", total_len);
        
                                let mut itteration = 1;
                                while itteration <= total_len{
                                    let val : [u8; 64] = match s_buf_clone.get(&(array_index as u32), 0){
        
                                        Ok(x) => {
                                            match x.get(cpu_id as usize){
                                                Some(y) => *y,
                                                None => [0;64],
                                            }
                                        },
                                        Err(_) => [0;64],
                                    };
                                    array_index += 1;
                                    println!("{}", array_index);
                                
                                    if val == [0;64]{
                                        filename.push(166 as char); // ¦ for empty spaces for debugging
                                        itteration += 1;
                                    }
                                    else{
                                        for &byte in val.iter().take_while(|&&c| c != 0) {
                                            filename.push(byte as char); // Convert non-null bytes to chars
                                            itteration += 1;
                                        }
                                        //filename.push(val as char); // Convert u8 to char and push to String
                                    }
                                }
                                // Correct reversed path
                                let corrected_path: String = filename
                                    .split('/')
                                    .rev()
                                    .collect::<Vec<&str>>()
                                    .join("/");
                                println!("Unreversed: {}", corrected_path);
        
                            
                                // Now we actually get to deal with deltas
                                // Create the final path from the path we got and the watch directory
                                let final_path = String::from(filter.get_base_dir()) + corrected_path.as_str();
                                let should_filter = filter.should_filter(final_path.as_str());
        
                                // Extract deltas
                                if !should_filter {
                                    // send_full_contents_of_file_tcp(final_path.as_str());
                                    client_tcp::write_full_file_to_connections(final_path.as_str());
                                    
                                }
                            },
                            1 => println!("vfs_mkdir"),
                            2 => println!("vfs_rmdir"),
                            3 => println!("vfs_rename"),
                            _ => panic!("Error: Undetermined Call"), // `_` is a catch-all pattern for any other case
                        }

                        // We have the 2 u8's that consist of a u16
                        // Even though get_u16() existed, it gave me really large numbers
                        // So I instead combined two u8's
                        // We know the max size is 4096 so we can use values higher than that as error codes
                        // Or 0
                        
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
