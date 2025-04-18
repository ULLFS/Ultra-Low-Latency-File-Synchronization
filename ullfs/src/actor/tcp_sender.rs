use std::{collections::HashMap, error::Error, time::Duration, sync::Arc};
// use async_std::stream;
use steady_state::*;
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::oneshot::{self, Receiver, Sender}, time::sleep, sync::Mutex};
use crate::{actor::ebpf_listener::ChangeType, client_tcp::{self, write_create_dir_to_connection, write_create_file_to_connection, write_deletion_to_connection, write_move_to_connection}, fileDifs::{self, FileData}, fileFilter, Args, TcpChannel};
use super::ebpf_listener::{RuntimeState, TcpData};

async fn resend_file(file: &String, stream: &mut TcpStream, name: String){
    println!("Resending File: {} to address: {}", file, "llfs.ullfs.com");
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
                                println!("resending: {}", "llfs.ullfs.com");
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
    let mut vec_tcp_streams: Arc<Mutex<Vec<(TcpStream, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let (mut cur_prod, mut cur_cons);
    // let mut prev_debounce = None;
    let mut old_data: Option<(Sender<u8>, TcpData)> = None;
    while cmd.is_running(&mut || ebpf_rx.is_closed_and_empty() && tcp_rx.is_closed_and_empty()) {

        match cmd.try_take(&mut tcp_rx) {
            Some(x) => {
                
                println!("Found a new stream: {}", "llfs.homiecourt.xyz");
                let mut lock_tcp_streams = vec_tcp_streams.lock().await;
                // println!("Found some tcp_streams: {}", x.len());
                lock_tcp_streams.push((x.stream, x.name));
                // for stream in x {
                //     vec_tcp_streams.push(stream);
                // }
                // vec_tcp_streams = x;
            }
            None => {}
        }
        let mut lock_read_streams = vec_tcp_streams.lock().await;
        read_streams(&mut lock_read_streams, &mut cmd, &mut map_filenames, &mut connection_handler_sender).await;
            
        match cmd.try_take(&mut ebpf_rx) {
            Some(data) => {
                // match data
                //Debounce Data:
                let filter_manager = fileFilter::Filter::get_instance();
                if filter_manager.should_filter(&data.filename){
                    continue;
                }
                (cur_prod, cur_cons) = oneshot::channel();
                let clone_data = data.clone();
                let streams = vec_tcp_streams.clone();
                match old_data{
                    Some((old_sender, info)) => {
                        if(info == data) {
                            old_sender.send(1u8); // Kill it
                        }
                    }
                    None => {
                        // old_data = Some((cur_prod, data.clone()));
                    }
                }
                // Create task:
                tokio::task::spawn(async move {
                    sleep(Duration::from_millis(100)).await;
                    match cur_cons.try_recv() {
                        Ok(_) => {
                            // Received a value so that means we kill this task
                            return;
                        }
                        Err(_) => {
                            // return;
                        }
                    }
                    // let 
                    println!("Data: {}-{}-{}", data.change_type as u8, data.filename, data.old_filename);
                    match data.change_type {
                        ChangeType::create_dir => {
                            let mut lock_streams = streams.lock().await;
                            for (stream, name) in &mut lock_streams.iter_mut() {
                                let base_dir = fileFilter::Filter::get_instance().get_base_dir();   
                                let filename = base_dir.to_string() + &data.filename;
                                write_create_dir_to_connection(&filename, stream).await;
                            }
                        }
                        ChangeType::delete => {
                            let mut lock_streams = streams.lock().await;

                            for (stream, name) in &mut lock_streams.iter_mut() {
                                let base_dir = fileFilter::Filter::get_instance().get_base_dir();   
                                let filename = base_dir.to_string() + &data.filename;
                                write_deletion_to_connection(&filename, stream).await;
                            }
                        }
                        ChangeType::create_file => {
                            let mut lock_streams = streams.lock().await;

                            for (stream, name) in &mut lock_streams.iter_mut(){
                                let base_dir = fileFilter::Filter::get_instance().get_base_dir();   
                                let filename = base_dir.to_string() + &data.filename;
                                write_create_file_to_connection(&filename, stream).await;
                            }
                        }
                        ChangeType::move_fdir => {
                            let mut lock_streams = streams.lock().await;

                            for (stream, name) in &mut lock_streams.iter_mut() {
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
                                let mut lock_streams = streams.lock().await;

                                for ( stream, name) in &mut lock_streams.iter_mut() {
                                    println!("Writing delta file to connection: {}", name);
                                    match delta {
                                        Some(ref x) => {
                                            client_tcp::write_delta_to_connection(x, &file, stream).await;
                                        }
                                        None => {}
                                    }

                                }
                            } else {
                                filemanager.add_file(file.to_string());
                                let mut lock_streams = streams.lock().await;

                                for (stream, name) in &mut lock_streams.iter_mut() {
                                    println!("Writing full file to connection: {}", name);
                                    
                                    client_tcp::write_full_file_to_connection(&file, stream).await;
                                }
                            }
                        }
                    }
                    // Ok(())
                });

                // Reset the producer and consumers
                old_data = Some((cur_prod, clone_data));
                // Ok(())


                
                
            }
            None => {
                // Ok(())
            }
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