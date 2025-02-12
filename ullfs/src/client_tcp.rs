use std::{borrow::BorrowMut, collections::HashMap, fs, io::{BufRead, BufReader}, net::TcpStream, sync::{OnceLock, RwLock}};

use serde_json::Value;


static INSTANCE :OnceLock<connections> = OnceLock::new();
struct connections {
    addresses: RwLock<HashMap<&'static String, &'static TcpStream>>,
    connections: u32,
    port: u32
}
impl connections {
    pub fn new() -> Self {
        
        connections {
            addresses: RwLock::new(HashMap::new()),
            connections: 0,
            port: 0
        }
    }
    pub fn get_instance() -> &'static connections{
        INSTANCE.get_or_init(|| connections::new())
    }
    pub fn check_connections_config(&self) {
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
        let addresses_conf = conf["dns_web_addresses"].as_array().expect("Dns_web_addresses not an array or was malformed");
        let mut addr = self.addresses.write().unwrap();
        for address in addresses_conf {
            let address_string = address.to_string();
            if addr.contains_key(&address_string){
                let stream = addr.get(&address_string).expect("Couldn't get TcpStream");
                
            } else {
                let stream_address = TcpStream::connect(address_string).expect("Failed to connect to server");
                let stream_static: &'static TcpStream = Box::leak(Box::new(stream_address));
                let address_static: &'static String = Box::leak(Box::new(address.to_string()));
                addr.insert(address_static, stream_static);
            }
        }
    }
}
pub fn write_to_connections(){
    
}

