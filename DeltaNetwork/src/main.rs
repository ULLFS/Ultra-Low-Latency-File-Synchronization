// use std::{fs, io::{stdin, Read}};

use client_tcp::write_delta_file_to_connections;
use ignore::Error;
use tokio::net::TcpStream;
use xxhash_rust::xxh3::xxh3_64;

mod client_tcp;
mod fileFilter;
mod fileDifs;

use std::{fs, future::Future, io::{stdin, stdout, BufReader, Read, Write}, path::Path};

fn pause() {
    let mut stdout = stdout();
    stdout.write(b"Press Enter to continue...").unwrap();
    stdout.flush().unwrap();
    stdin().read(&mut [0]).unwrap();
}

async fn test_deltas(){
    let base_dir = "/home/zmanjaroschool/TestDir2/";
    let full_file = "/home/zmanjaroschool/TestDir2/new_folder/test.txt";
    let tests = ["test_add.txt", "test_remove.txt", "test_replace.txt", "test_really_long.txt", "test_add_to_end.txt","specific_event.txt", "specific_event_b.txt", "test_add_to_start.txt", "test_add_single_char.txt"];
    let mut full_file_fs = fs::File::open(full_file).expect("Failed to open full file");
    let mut file_data: String = String::new();
    let mut stream = TcpStream::connect("127.0.0.1:34254").await.expect("Failed to connect to server");
    full_file_fs.read_to_string(&mut file_data).expect("Failed to read data to string");
    let mut dif_manager = fileDifs::FileData::get_instance();
    dif_manager.add_file(full_file.to_string());
    // let mut old_file_data_u8 = file_data.into_bytes();
    println!("Writing to connections");
    client_tcp::write_full_file_to_connections(full_file, &mut stream).await;
    println!("Finished");
    pause();
    for test in tests {
        println!("Test: {}", test);
        let filepath = base_dir.to_string() + test;
        // let mut file = fs::File::open(&filepath).expect(format!("Failed to read test file: {}", test).as_str());
        // let mut new_file_data: String = String::new();
        // file.read_to_string(&mut new_file_data).expect("Failed to read data to string");
        // let new_file_data_u8: Vec<u8>;
        // new_file_data_u8 = new_file_data.into_bytes();

        // let old_file_data_u8 = file_data.into_bytes();
        // let delta = fileDifs::get_delta(&old_file_data_u8, &new_file_data_u8);
        let delta = dif_manager.get_file_delta(full_file, &filepath);
        // old_file_data_u8 = new_file_data_u8;
        write_delta_file_to_connections(&delta, full_file, &mut stream).await;
        pause();
        
        
    }
    
}
#[tokio::main]
async fn main() {
    test_deltas().await;
}
