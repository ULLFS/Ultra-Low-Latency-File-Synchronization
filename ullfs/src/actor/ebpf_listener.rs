
use aya::{include_bytes_aligned, maps::{Array, AsyncPerfEventArray, PerCpuArray}, programs::KProbe, util::online_cpus, Ebpf};
use bytes::{Buf, BytesMut};
#[allow(unused_imports)]
use log::*;
use serde_json::Value;
/* use tokio::runtime::Runtime;
use std::default; */
#[allow(unused_imports)]
use std::time::Duration;
use steady_state::*;

// use crate::Args;
use std::{error::Error, fs, io::BufReader, process, sync::{Arc}};
//use crate::actor::tcp_worker::TcpResponse;
use tokio::{net::{TcpListener, TcpStream}, sync::Mutex, sync::mpsc};

use crate::{ebpf_setup::ebpf_setup, fileFilter};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
//use std::io::{Read,Write};
//use std::sync::Arc;
// use tokio::time::Sleep;

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

pub async fn run(context: SteadyContext
    ,transmitter: SteadyTx<Box<String>>
    ,state: SteadyState<RuntimeState>
) -> Result<(),Box<dyn Error>> {
    // let mut cmd_list = Vec::new();
    // for transmitter in &transmitter_list {
    //     let cmd =  into_monitor!(context.clone(), [],[transmitter]);
    //     // internal_behavior(cmd, transmitter, state).await
    //     cmd_list.push(cmd);

    // }
    internal_behavior(context, transmitter, state).await

}

async fn internal_behavior(context: SteadyContext, 
    transmitter: SteadyTx<Box<String>>,
    state: SteadyState<RuntimeState>) -> Result<(),Box<dyn Error>>{
    let mut state_guard = steady_state(&state, || RuntimeState::new(1)).await;

    let mut cmd =  into_monitor!(context, [],[transmitter]);

    if let Some(mut _state) = state_guard.as_mut() {
        // let mut tcp_msg_rx = tcp_msg_rx.lock().await;
        // println!("Running ebpf_builder"); // Why is ebpf_builder running twice?

        let mut bpf = ebpf_setup()?;
        let mut perf_array = AsyncPerfEventArray::try_from(bpf.take_map("EVENTS").unwrap()).expect("Failed to set up perf event array");
        let buf_array: PerCpuArray<_,[u8;64]> = match PerCpuArray::try_from(bpf.take_map("BUF").unwrap()){
            Ok(x) => x,
            Err(_) => {
                panic!("PerCpuArray failed to initialize");
            }
        };
        let s_buf = Arc::new(buf_array);
        // let watch_dir_clone = Arc::new(watch_dir);
        // let cmd_mutex = Arc::new(Mutex::new(cmd));
        // let mut i = 0;
        let (tx, mut rx) = mpsc::channel(100);
        for cpu_id in online_cpus().map_err(|(_, error)| error).expect("Failed to get online cpus") {
            // open a separate perf buffer for each cpu
            // let mut cmd =  into_monitor!(context.clone(), [],[transmitter]);cd
            // let mut transmitter_locked = transmitter.lock().await;
            // i += 1;
            let mut buf = perf_array.open(cpu_id, None).expect("Failed to open perf array");
            let s_buf_clone = Arc::clone(&s_buf);
            // let cmd_mutex_clone = Arc::clone(&cmd_mutex);
            // println!("About to lock:");
            // let mut ebpf_tx = transmitter.lock().await;
            let tx_clone = tx.clone();

            tokio::task::spawn(async move {
                
                println!("Spawned a task");
                
                let filter: &fileFilter::Filter = fileFilter::Filter::get_instance();

                let mut buffers = (0..4)
                    .map(|_| BytesMut::with_capacity(4))
                    .collect::<Vec<_>>();
                
                loop {

                    // println!("WEE");
                    // wait for events
                    let event = buf.read_events(&mut buffers).await.unwrap(); /*{
                        Some(x) => x,
                        None => {
                            panic!("Buffer read events bad somehow");
                        }
                    };*/
                    
                    // println!("Received event");
                    
                    for i in 0..event.read {
                        let buf = &mut buffers[i];
                        let len = buf.get_u8();
                        let len2 = buf.get_u8();
                        let data = buf.get_u8();

                        // println!("{}",data);
                        
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
                                // println!("totalLen: {}", total_len);
        
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
                                    // println!("{}", array_index);
                                
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
                                // println!("Unreversed: {}", corrected_path);
        
                            
                                // Now we actually get to deal with deltas
                                // Create the final path from the path we got and the watch directory
                                let final_path = String::from(filter.get_base_dir()) + corrected_path.as_str();
                                let should_filter = filter.should_filter(final_path.as_str());
                                // ebpf_listener_conn_tx.send("Test").await.unwrap();
                                // Extract deltas
                                if !should_filter {
                                    // send_full_contents_of_file_tcp(final_path.as_str());
                                    // client_tcp::write_full_file_to_connections(final_path.as_str());
                                    match tx_clone.send(final_path).await {
                                        Ok(x) => x,
                                        Err(e) => {
                                            println!("Failed to send along tx: {}", e);
                                        }
                                    };
                                    // cmd.send_async(&mut transmitter_locked, "test".to_string(), SendSaturation::IgnoreAndWait);
                                    // match cmd_mutex_clone.lock().await.send_async(&mut ebpf_tx, final_path, SendSaturation::IgnoreAndWait).await{
                                    //     Ok(x) => x,

                                    //     Err(e) => {
                                    //         println!("Got an error from send_async: {}", e);
                                    //         // Not panicing because maybe its a nonissue?
                                    //     }
                                    // };
                                    // println!("Out of lock");
                                }
                            },
                            1 => println!("vfs_mkdir"),
                            2 => println!("vfs_rmdir"),
                            3 => println!("vfs_rename"),
                            _ => panic!("Error: Undetermined Call"), // `_` is a catch-all pattern for any other case
                        }
                    }
                }
                // println!("Ended loop");
                // return;
            });
            
        }
        let mut received_data = rx.recv().await;
        // let mut received_string = match received_data {
        //     Some(x) => x,
        //     None => "FAILURE".to_string()
        // };
        // println!("Received some data: {}", received_string);

        loop{
            // This begins the weird translation between tokio and steady state
            // It exists purely because Steady State does not work with Aya alone
            // We needed multi transmitters and a single receiver.
            // All those transmitters send there data here, which will then send off to other actors
            // println!("{}", received_string);
            let mut received_string = match received_data {
                Some(x) => x,
                None => {
                    println!("Received none, end of receivers"); // Hopefully this code works
                    break;
                }
            };
            println!("Data Recieved: {}", received_string);
            let mut transmit_lock = transmitter.lock().await;
            cmd.send_async(&mut transmit_lock, Box::new(received_string), SendSaturation::IgnoreAndWait).await;
            received_data = rx.recv().await;

        }
    }
    println!("Finished!");
    
    // loop{}
    
    Ok(())
}
