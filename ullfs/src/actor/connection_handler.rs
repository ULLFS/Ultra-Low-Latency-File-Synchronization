use std::{collections::HashMap, error::Error, fs, io::BufReader, ops::Index, time::Duration};

use serde_json::Value;
use steady_state::*;
use tokio::{net::{unix::SocketAddr, TcpSocket, TcpStream}, time::{timeout, sleep}};

use super::ebpf_listener::RuntimeState;
fn resend_data(input_data: Vec<u8>){

}
fn get_connection_addresses() -> (Vec<String>, String){
    println!("Checking connections config");
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
            panic!("Error: config.json structure damaged.\n{}", e);
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
async fn check_connections_config(tcpstreams:&mut Vec<(TcpStream, String)>, disconnected: &Vec<String>) -> Vec<String>{
    let (mut addresses_string, port) = get_connection_addresses();
    tcpstreams.retain(| (stream, address)| {
        let mut buf = Vec::new();
        let num_recvd = match stream.try_read(&mut buf){
            Ok(x) => {
                resend_data(buf);
                x
            }
            Err(_) => {
                // An error means that we got no data, not that the connection had an issue
                1
            }
        };
        // if the number of received bytes is 0 that is TCP telling us the connection was dropped
        // If the config.json changed which addresses to look at and this address was not in our
        // new data, drop the connection
        let mut output = num_recvd != 0;
        if(!output){
            addresses_string.retain(|adr| {
                // Removing the addresses that are still good
                if address == adr {
                    output = true;
                }
                address != adr
            });
        }
        
        output
    });
    let mut output = Vec::new();
    for address in addresses_string {
        if !disconnected.contains(&address.to_string()) {
            continue;
        }
        let stream_future = TcpStream::connect(address.to_string() + ":" + port.as_str());
        // Only allow 10 seconds for each connection to establish and if not established give up
        // This way we aren't waiting forever for a connection
        println!("Starting timeout");
        let stream = match timeout(Duration::from_secs(10), stream_future).await {
            Ok(x) => {
                match x {
                    Ok(e) => e,
                    Err(e) => {
                        println!("Failed to connect to address: {}:{}, {}", address, port,e );
                        output.push(address);
                        continue;
                    }
                }
            }
            Err(_) => {
                continue;
            }
        };
        tcpstreams.push((stream, address.to_string()));
    }
    return output;


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
pub async fn run(context: SteadyContext, 
    transmitter: SteadyTx<Vec<(TcpStream, String)>>,
    receiver: SteadyRx<Box<String>>,
    state: SteadyState<RuntimeState>) -> Result<(),Box<dyn Error>> {
    
    let mut cmd = into_monitor!(context, [], [transmitter]);
    let mut file_to_resend: HashMap<String, String> = HashMap::new();
    let mut tx = transmitter.lock().await;
    let mut rx = receiver.lock().await;
    // Disconnected addresses should start as all of them
    let (mut disconnected, _) = get_connection_addresses();
    
    while cmd.is_running(&mut | | {
        tx.mark_closed() && rx.is_closed_and_empty()
    }) {
        // let recheck_connections = poll_connections(&mut connections, &mut file_to_resend).await;
        // if recheck_connections {
        // let mut connections_clone = Vec::new();
        // for (connection, name) in connections{
        //     connections_clone.push((connection.clone(), name));
        // }
        let mut connections = Vec::new();

        disconnected = check_connections_config(&mut connections, &disconnected).await;
        // Sends over all new connections that should be held
        cmd.send_async(&mut tx, connections, SendSaturation::IgnoreAndWait).await;
        
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
        
        sleep(Duration::from_secs(300)).await;
        // }
    }
    Ok(())
}