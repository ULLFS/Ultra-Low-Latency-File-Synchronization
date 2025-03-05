use std::{borrow::BorrowMut, collections::HashMap, fs, future::Future, io::{BufRead, BufReader, Read}, sync::{OnceLock, RwLock}, task::Poll};

// use async_std::{io::WriteExt, net::TcpStream};
use serde_json::Value;
use tokio::{io::AsyncWriteExt, net::{TcpListener, TcpStream}, runtime::Runtime, task::{self, block_in_place}};
// use async_std::io::{Write, Read};
use crate::{fileDifs, fileFilter};


static INSTANCE :OnceLock<Connections> = OnceLock::new();
struct Connections {
    addresses: RwLock<HashMap<&'static String, &'static mut TcpStream>>,
    connections: u32,
    port: u32
}
impl Connections {
    pub fn new() -> Self {
        
        let c = Connections {
            addresses: RwLock::new(HashMap::new()),
            connections: 0,
            port: 0
        };
        // let rt = Runtime::new().unwrap();
        task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(c.check_connections_config())
        });
        // rt.block_on();
        c
        
    }
    pub async fn get_instance() -> &'static Connections{
        INSTANCE.get_or_init(||Connections::new())
    }
    pub async fn check_connections_config(&self) -> bool {
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
        let port: &str = conf["server_port"].as_str().expect("Port was not actually a string");
        let addresses_conf = conf["dns_web_addresses"].as_array().expect("Dns_web_addresses not an array or was malformed");
        ;
        for address in addresses_conf {
            let address_string = address.as_str().expect("was not a string somehow").to_string() + ":" + port;
            let mut addr = self.addresses.write().unwrap();
            if addr.contains_key(&address_string){
                let stream = addr.get(&address_string).expect("Couldn't get TcpStream");
                
            } else {
                match TcpStream::connect(&address_string).await {
                     Ok(x) => {
                        
                        let stream_static: &'static mut TcpStream = Box::leak(Box::new(x));
                        let address_static: &'static String = Box::leak(Box::new(address.to_string()));
                        addr.insert(address_static, stream_static);
                     }
                     Err(x) => {
                         println!("Failed to connect to server {}, {}", address_string, x);
                     }
                 }
                
            }
        }
        return true;
    }
}
pub async fn write_full_file_to_connections(filepath: &str){
    println!("Writing to connections:");
    let mut addr = Connections::get_instance().await.addresses.write().unwrap();
    for (address, mut connection) in addr.iter_mut() {
        let mut c = connection;
        println!("Writing to {}", address);
        // match c.read(&mut []){
        //     Ok(x) => {}
        //     Err(_) => {
        //         Connections::get_instance().check_connections_config();
                
        //     }
        // }
        // println!("c.read complete");
        let base_path = fileFilter::Filter::get_instance().get_base_dir();
        let relative_path = filepath.replace(base_path, ""); // Removing base path from the file path to get relative path
        let mut relative_path_bytes = relative_path.into_bytes();
        relative_path_bytes.push(0b0000);
        // Relative path now ends with a null byte which will never be allowed in a file name
        // This is one of two characters that are completely illegal
        relative_path_bytes.push(1u8);
        // Push identifier for the full file send
        match c.write(relative_path_bytes.as_slice()).await{
            Ok(x) => x,
            Err(x) => {
                println!("Error on writing relative path: {} on connection address: {}",x, address);
                return;
            }
        };
        
        let mut buf = [0u8; 1024];
        let mut f = fs::File::open(filepath).expect(format!("File not found somehow: {}", filepath).as_str());
        let file_length = f.metadata().unwrap().len();
        println!("File length: {}", file_length);
        c.write(&file_length.to_le_bytes()).await.expect("Failed to write file length");
        // let mut reader = BufReader::new(f);
        loop {
            let num_bytes = f.read(&mut buf).expect(format!("Failed to read file: {}", filepath).as_str());
            match c.write(&buf[..num_bytes]).await{
                Ok(x) => x,
                Err(x) => {
                    println!("Failed to write to connection: {} while writing file data. Error: {}", address, x);
                    return;
                }
            };
            if num_bytes == 0 {
                break;
            }
        }
        
        
    }
}
pub async fn write_delta_file_to_connections(delta: &fileDifs::Delta, filepath: &str){
    let mut addr = Connections::get_instance().await.addresses.write().unwrap();
    for (address, connection) in addr.iter_mut() {
        let mut c = connection;
        println!("Writing to {}", address);
        // match c.read(&mut []){
        //     Ok(x) => {}
        //     Err(_) => {
        //         Connections::get_instance().check_connections_config();
                
        //     }
        // }
        // println!("c.read complete");
        let base_path = fileFilter::Filter::get_instance().get_base_dir();
        let relative_path = filepath.replace(base_path, ""); // Removing base path from the file path to get relative path
        println!("{}", relative_path);
        let mut relative_path_bytes = relative_path.into_bytes();
        relative_path_bytes.push(0b0000);
        relative_path_bytes.push(2u8);
        let data_len = delta.data.len();
        println!("Start index: {}", delta.start_index);
        for byte in delta.start_index.to_le_bytes() {
            relative_path_bytes.push(byte);
        }
        println!("End index: {}", delta.end_index);
        
        for byte in delta.end_index.to_le_bytes() {
            relative_path_bytes.push(byte);
        }
        println!("Data length: {}", data_len);
        for byte in data_len.to_le_bytes() {
            relative_path_bytes.push(byte);            
        }
        println!("Hash: {}", delta.old_hash);
        for byte in delta.old_hash.to_le_bytes() {
            println!("Byte: {}", byte);
            
            relative_path_bytes.push(byte);
        }
        for byte in delta.data.iter() {
            relative_path_bytes.push(*byte);
        }
        c.write(&relative_path_bytes).await;
        // relative_path_bytes.push(delta.start_index.to_le_bytes());
        // c.flush();

        
                        
    }
}

