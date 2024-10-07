use aya::programs::KProbe;
use aya::{
    include_bytes_aligned,
    Bpf,
    maps::{HashMap,Array},
};
use aya_log::BpfLogger;
use log::{info, warn, debug};
use tokio::signal;
use std::fs;
use std::os::linux::raw;
use std::io::BufReader;
use serde_json::{self, Value};
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
    let mut bpf = Bpf::load(include_bytes_aligned!(
        "../../target/bpfel-unknown-none/debug/ullfs"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut bpf = Bpf::load(include_bytes_aligned!(
        "../../target/bpfel-unknown-none/release/ullfs"
    ))?;
    if let Err(e) = BpfLogger::init(&mut bpf) {
        // This can happen if you remove all log statements from your eBPF program.
        warn!("failed to initialize eBPF logger: {}", e);
    }
    let program: &mut KProbe = bpf.program_mut("vfs_write").unwrap().try_into()?;
    program.load()?;
    program.attach("vfs_write", 0)?;
    let conf_file : fs::File = fs::File::open("./config.json")?;
    let reader = BufReader::new(conf_file);
    let conf : Value = serde_json::from_reader(reader)?;
    let watch_dir : &str = match &conf["watch_dir"].as_str() {
        None => "~/",
        Some(x) => x,
    };
    println!("Conf file output: {}", &watch_dir);
    let w_dir = match (fs::metadata(watch_dir)){
        Ok(x) => x,
        Err(e) => {
            panic!("Error: Directory {} not found, something must be wrong with your config file\n{}", &watch_dir, e);
        }
    };
    let block_addr: u64 = std::os::linux::fs::MetadataExt::st_ino(&w_dir);
    let mut inodesdata: Array<_, u64> = Array::try_from(bpf.map_mut("INODEDATA").unwrap())?;
    println!("Inode found at inode: {}", &block_addr);
    // let block_addr: u64 = st_ino(watch_dir);
    // let block_addr: u64 = 31085353; 

    //{Index, Value, Flags}
    inodesdata.set(0, block_addr, 0)?;

    info!("Waiting for Ctrl-C...");
    signal::ctrl_c().await?;
    info!("Exiting...");

    Ok(())
}
