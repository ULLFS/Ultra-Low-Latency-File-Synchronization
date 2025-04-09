use std::{collections::HashMap, error::Error, sync::{Arc, Mutex}};
// use async_std::stream;
use steady_state::*;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use crate::{actor::ebpf_listener::ChangeType, client_tcp::{self, write_create_dir_to_connection, write_create_file_to_connection, write_deletion_to_connection, write_move_to_connection}, fileDifs::{self, FileData}, fileFilter, Args, TcpChannel};
use super::ebpf_listener::{RuntimeState, TcpData};

async fn resend_file(file: &String, stream: &mut TcpStream, name: String){
    println!("Resending File: {} to address: {}", file, name);
    client_tcp::write_full_file_to_connection(file, stream).await;
}
// fn create_full_filename(file: &String) -> String {
//     let startDir = 
// }

async fn read_streams <C: SteadyCommander>(
    streams: &mut Vec<(TcpStream, String)>,
    cmd: &mut C,
    map_filenames: &mut HashMap<String, String>,
    conn_tx: &mut SteadyTx<String>
)
    {
        
    // let mut vec_streams_temp: Vec<(TcpStream, String)> = Vec::new();
    let mut vec_disconnected: Vec<String> = Vec::new();
    for (stream, name) in streams.iter_mut() {
        let mut buf: Vec<u8> = Vec::new();
        let lost_connection: bool = match stream.try_read(&mut buf) {
            Ok(x) => {
                if(x != 0) {
                    println!("Received a stream to resend")
                }
                for byte in buf{
                    let name = name.clone();
                    if byte == 0b0000 {
                        // 0 byte means the end of a filepath
                        let val = map_filenames.get(&name);
                        match val {
                            Some(x) => {
                                println!("resending: {}", name);
                                resend_file(x, stream, name).await;
                            }
                            None =>{}
                        };
                        
                        // resend_file(map_filenames.get(&name.as_str()));
                    } else {
                        map_filenames.entry(name).or_insert_with(String::new).push(byte as char);
                    }
                }
                x == 0
            }
            Err(_) => {

                false
            }
        };
        if !lost_connection {
            // vec_streams_temp.push((stream, name.to_string()));

        } else {
            vec_disconnected.push(name.to_string());
            let mut tx = conn_tx.lock().await;
            let _ = cmd.send_async(&mut tx, name.to_string(), SendSaturation::IgnoreAndWait).await;
            cmd.relay_stats();
        }
    };
    streams.retain(|(_stream, name)| {
        if vec_disconnected.contains(name) {
            false
        } else {
            true
        }
    });
    // return streams;
}

pub async fn run(context: SteadyContext,
    ebpf_receiver: SteadyRx<TcpData>,
    tcp_receiver: SteadyRx<TcpChannel>,
    connection_handler_sender: SteadyTx<String>,
    state: SteadyState<RuntimeState>) -> Result<(),Box<dyn Error>>{
        
    // if needed CLI Args can be pulled into state from _cli_args
    let _cli_args = context.args::<Args>();
    // monitor consumes context and ensures all the traffic on the chosen channels is monitored
    // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
    let cmd =  into_monitor!(context, [ebpf_receiver, tcp_receiver],[connection_handler_sender]);
    internal_behavior(cmd, ebpf_receiver,tcp_receiver,connection_handler_sender,state).await
    // loop {}
    // Ok(())

}

async fn internal_behavior <C: SteadyCommander>(
    mut cmd: C, 
    ebpf_receiver: SteadyRx<TcpData>,
    tcp_receiver: SteadyRx<TcpChannel>,
    mut connection_handler_sender: SteadyTx<String>,
    state: SteadyState<RuntimeState>,
) -> Result<(),Box<dyn Error>> {
    //Expects received data to be in the following format:
    // Send_type\0Filepath
    // Send Type can be FULL_FILE, CREATE_DIRECTORY, RENAME_DIR, RENAME_FILE,
    // DELTA_FILE, DELETE_FILE, DELETE_DIR, AUTO_FILE
    // More to come possibly
    // AUTO_FILE specifically will automatically determine whether to send a delta file or full file
    //let mut cmd = into_monitor!(context, [ebpf_receiver, tcp_receiver], [connection_handler_sender]);
    
    let mut ebpf_rx = ebpf_receiver.lock().await;
    let mut tcp_rx = tcp_receiver.lock().await;


    let mut map_filenames: HashMap<String, String> = HashMap::new();
    let mut vec_tcp_streams: Vec<(TcpStream, String)> = Vec::new();

    while cmd.is_running(&mut || ebpf_rx.is_closed_and_empty() && tcp_rx.is_closed_and_empty()) {

        match cmd.try_take(&mut tcp_rx) {
            Some(x) => {
                println!("Found a new stream: {}", x.name);
                // println!("Found some tcp_streams: {}", x.len());
                vec_tcp_streams.push((x.stream, x.name));
                // for stream in x {
                //     vec_tcp_streams.push(stream);
                // }
                // vec_tcp_streams = x;
            }
            None => {}
        }
        
        read_streams(&mut vec_tcp_streams, &mut cmd, &mut map_filenames, &mut connection_handler_sender).await;
            
        match cmd.try_take(&mut ebpf_rx) {
            Some(data) => {
                // match data
                println!("Data: {}-{}-{}", data.change_type as u8, data.filename, data.old_filename);
                match data.change_type {
                    ChangeType::create_dir => {
                        for (stream, name) in &mut vec_tcp_streams {
                            let base_dir = fileFilter::Filter::get_instance().get_base_dir();   
                            let filename = base_dir.to_string() + &data.filename;
                            write_create_dir_to_connection(&filename, stream).await;
                        }
                    }
                    ChangeType::delete => {
                        for (stream, name) in &mut vec_tcp_streams {
                            let base_dir = fileFilter::Filter::get_instance().get_base_dir();   
                            let filename = base_dir.to_string() + &data.filename;
                            write_deletion_to_connection(&filename, stream).await;
                        }
                    }
                    ChangeType::create_file => {
                        for (stream, name) in &mut vec_tcp_streams {
                            let base_dir = fileFilter::Filter::get_instance().get_base_dir();   
                            let filename = base_dir.to_string() + &data.filename;
                            write_create_file_to_connection(&filename, stream).await;
                        }
                    }
                    ChangeType::move_fdir => {
                        for (stream, name) in &mut vec_tcp_streams {
                            let base_dir = fileFilter::Filter::get_instance().get_base_dir();
                            let filename = base_dir.to_string() + &data.filename;
                            let old_filename = base_dir.to_string() + &data.old_filename;
                            write_move_to_connection(&old_filename, &filename, stream ).await;

                        }
                    }
                    ChangeType::write => {
                        let base_dir = fileFilter::Filter::get_instance().get_base_dir();
                        let file = base_dir.to_string() + &data.filename;
                        // let file = data.filename;
                        println!("Received a file to send: {}", file);
                        let filemanager = fileDifs::FileData::get_instance();
                        if filemanager.contains_file(&file) {
                            // Send the delta:
                            let delta = filemanager.get_file_delta(&file);
                            for ( stream, name) in &mut vec_tcp_streams {
                                println!("Writing delta file to connection: {}", name);
                                client_tcp::write_delta_to_connection(&delta, &file, stream).await;

                            }
                        } else {
                            filemanager.add_file(file.to_string());
                            for (stream, name) in &mut vec_tcp_streams {
                                println!("Writing full file to connection: {}", name);
                                
                                client_tcp::write_full_file_to_connection(&file, stream).await;
                            }
                        }
                    }
                }
                
            }
            None => {}
        };
        // cmd.relay_stats();
    }
    

    Ok(())
} 


// async fn read_streams_wrapper(
//     vec_tcp_streams: &mut Vec<(TcpStream, String)>,
//     cmd: &mut LocalMonitor<2,1>,
//     map_filenames: &mut HashMap<String, String>,
//     conn_tx: &mut futures_util::lock::MutexGuard<'_, Tx<Box<String>>>,
// ) {
//     read_streams(vec_tcp_streams, cmd, map_filenames, conn_tx).await;
// }