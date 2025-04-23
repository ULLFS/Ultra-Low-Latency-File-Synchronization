use std::{fs, io::BufReader, process};

use anyhow::Error;
use aya::{include_bytes_aligned, maps::Array, programs::KProbe, Ebpf};
use log::debug;
use serde_json::Value;

pub fn ebpf_setup() -> Result<Ebpf, Error> {
// let dif = fileDifs::FileData::get_instance();
    // let old = dif.get_file_delta("/home/zmanjaroschool/TestDir/testDif.txt");
    // let t = tokio::signal::ctrl_c().await;
    // let new = dif.get_file_delta("/home/zmanjaroschool/TestDir/testDif.txt");
    // println!("Old size: {}. New size: {}", old.1.len(), new.1.len());
    // println!("New start: {}. New end: {}", new.0, new.2);
    // return Ok(());
    // env_logger::init();
    
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
    
        
    // let mut bpf = ebpf_builder();
    // l
        // tokio::time::sleep
        // let mut bpf = ebpf_builder().expect("Failed to set up ebpf code");
    Ok(bpf)

    // panic!("");
}