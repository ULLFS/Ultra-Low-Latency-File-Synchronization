use std::{error::Error, fs, io::BufReader, time::Duration};
use serde_json::Value;
use steady_state::*;
use tokio::{net::TcpStream, time::{sleep, timeout}};
use crate::{client_tcp, Args, TcpChannel};
use super::ebpf_listener::RuntimeState;
fn get_zero_byte_string(buffer: Vec<u8>, old_unfinished: &str) -> (Vec<String>, String) {
    // Grabbing zero byte strings as well as returning the unfinished one if we have only gotten part of one
    let mut cur_str = old_unfinished.to_string();
    let mut vec_strings = Vec::new();
    for byte in buffer {
        if byte == 0u8 {
            vec_strings.push(cur_str);
            cur_str = String::new();
        } else {
            cur_str.push(byte as char);
        }
    }


    (vec_strings, cur_str)

}

fn get_connection_addresses() -> (Vec<String>, String){
    // println!("Checking connections config");
    // Begin dealing with the config file
    let conf_file : fs::File = match fs::File::open("./config.json"){
        Ok(x) => x,
        Err(e) => {
            
            panic!("Error: config.json missing or destroyed.\n{}", e)
        }
    };
    
    let reader = BufReader::new(conf_file);
    let conf : Value = match serde_json::from_reader(reader){
        Ok(x) => x,
        Err(e) => {
            panic!("Error: config.json structure damaged.\n{}",e);
        }
    };
    let port: String = conf["server_port"].as_str().expect("Failed to get port as string").to_string();
    let addresses_conf = conf["dns_web_addresses"].as_array().expect("Dns_web_addresses not an array or was malformed");
    // Clearing out old 
    let addresses_string: Vec<String>= addresses_conf.into_iter().map(|element| {
        element.as_str().expect("Failed to get address as string").to_string()
    }).collect();
    return (addresses_string, port);
}

async fn check_connections_config(tcpstreams:&mut Vec<(TcpStream, String, String)>, disconnected: &Vec<String>) -> Vec<String>{
    let (mut addresses_string, port) = get_connection_addresses();
    let mut resend_streams: Vec<(usize, Vec<String>, String)> = Vec::new();
    let i = 0;
    tcpstreams.retain_mut(| (stream, address, unfinished_str)| {
        let mut buf = Vec::new();
        let num_recvd = match stream.try_read(&mut buf){
            Ok(x) => {
                let (files, unfinished) = get_zero_byte_string(buf, unfinished_str);
                // unfinished_str = unfinished;
                resend_streams.push((i, files, unfinished));
                // resend_data(buf, stream);
                x
            }
            Err(_) => {
                // An error means that we got no data, not that the connection had an issue
                // So we set the number of bytes received to be something other than 0
                1
            }
        };
        // if the number of received bytes is 0 that is TCP telling us the connection was dropped
        // If the config.json changed which addresses to look at and this address was not in our
        // new data, drop the connection
        let mut output = num_recvd != 0;
        if !output {
            println!("Keeping address: {}", address);
            // Remove the addresses that we still have a connection to
            addresses_string.retain(|adr| {
                // Removing the addresses that are still good
                if address == adr {
                    output = true;
                }
                address != adr
            });
        } else {
            println!("Getting rid of address: {}", address);
        }
        // Drop the stream from the list if it was lost
        output
    });
    let mut output = Vec::new();
    for address in addresses_string {
        println!("Address checking for connection: {}", address);
        // Make sure the list of disconnected streams contains the stream before we go to readd it.
        if !disconnected.contains(&address.to_string()) {
            println!("Not disconnected");
            continue;
        }
        // Attempt to connect to the stream
        let stream_future = TcpStream::connect(address.to_string() + ":" + port.as_str());
        // Only allow 10 seconds for each connection to establish and if not established give up
        // This way we aren't waiting forever for a connection if there are issues. Most of the time this is done within a couple ms
        println!("Starting timeout");
        let stream = match timeout(Duration::from_secs(10), stream_future).await {
            Ok(x) => {
                match x {
                    Ok(e) => e,
                    Err(_e) => {
                        println!("Failed to connect to address: {}:{}, {}", address, port, _e);
                        output.push(address);
                        continue;
                    }
                }
            }
            Err(_) => {
                continue;
            }
        };
        tcpstreams.push((stream, address.to_string(), String::new()));
    }
    for (stream_id, files, unfinished_file) in resend_streams {
        for file in files {
            client_tcp::write_full_file_to_connection(&file, &mut tcpstreams[stream_id].0).await;
            tcpstreams[stream_id].2 = unfinished_file.clone();

        }
    }
    return output;


}

pub async fn run(context: SteadyContext
    , transmitter: SteadyTx<TcpChannel>
    , receiver: SteadyRx<String>
    , state: SteadyState<RuntimeState>) -> Result<(),Box<dyn Error>> {

    // if needed CLI Args can be pulled into state from _cli_args
    let _cli_args = context.args::<Args>();
    // monitor consumes context and ensures all the traffic on the chosen channels is monitored
    // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring

    let cmd = into_monitor!(context, [receiver], [transmitter]);
    // while cmd.is_running(|| {
    //     true
    // }) {}
    // Ok(())
    // let cmd =  into_monitor!(context, [receiver],[transmitter]);
    internal_behavior(cmd,transmitter,receiver,state).await
}

async fn internal_behavior <C: SteadyCommander>(
    mut cmd: C, 
    transmitter: SteadyTx<TcpChannel>,
    receiver: SteadyRx<String>,
    _state: SteadyState<RuntimeState>
) -> Result<(),Box<dyn Error>> {
    
    // let file_to_resend: HashMap<String, String> = HashMap::new();

    let mut tx = transmitter.lock().await;
    let mut rx = receiver.lock().await;

    // Disconnected addresses should start as all of them
    let (mut disconnected, _) = get_connection_addresses();
    
    while cmd.is_running(&mut || tx.mark_closed() && rx.is_closed_and_empty()) {
        // let recheck_connections = poll_connections(&mut connections, &mut file_to_resend).await;
        // if recheck_connections {
        // let mut connections_clone = Vec::new();
        // for (connection, name) in connections{
        //     connections_clone.push((connection.clone(), name));
        // }
        println!("Checking connections");
        let mut connections = Vec::new();

        disconnected = check_connections_config(&mut connections, &disconnected).await;

        // Sends over all new connections that should be held
        for connection in connections {
            let connection_struct: TcpChannel = TcpChannel { stream: connection.0, name: connection.1 };
            let _ = cmd.send_async(&mut tx, connection_struct, SendSaturation::IgnoreAndWait).await;
        }
        cmd.relay_stats();

        
        // As long as there is something to read, keep reading
        loop {
            match cmd.try_take(&mut rx) {
                Some(x) => {
                    // if we have disconnected, add it to the list
                    disconnected.push(x.to_string());
                }
                None => {
                    break;
                }
            }
        }
        
        sleep(Duration::from_secs(10)).await;
        // }
    }
    Ok(())
}

// async fn poll_connections(connections:&mut Vec<(TcpStream, String)>, file_to_resend: &mut HashMap<String, String>) -> bool {
//     let mut recheck_connections = false;
//     for (stream, address) in connections.iter_mut() {
//         let mut buf = Vec::new();
//         let num_bytes = match stream.try_read(&mut buf) {
//             Ok(x) => x,
//             Err(_) => {
//                 // Skip streams with nothing to read
//                 // uh oh we are polling
//                 // Maybe in the future we can have a timer where we only read like this for so long
//                 continue;
//             }
//         };
//         if num_bytes == 0 {
//             // A connection was dropped, check the connections config
//             recheck_connections = true;
//             // check_connections_config(&mut connections);
//         } else {
//             // We got a request for a new file
//             if file_to_resend.contains_key(address.as_str()){
//                 let new_addr = address.as_str();
//                 // let clone_addr = new_addr.clone();
//                 // let new_addr = clone_addr.as_str();
//                 for byte in buf {
//                     if(byte == b'\0'){
//                         // // file_to_resend.insert(new_addr, "".to_string());
//                         file_to_resend.remove(new_addr);
//                         // file_to_resend.insert(new_addr, String::new());

//                         // file_to_resend.insert(&address,  + byte as char);
//                     } else {
//                         file_to_resend.entry(address.clone()).or_insert(String::new()).push(byte as char);
//                     }
//                 }
//             }
//         }
//     }
//     recheck_connections
// }