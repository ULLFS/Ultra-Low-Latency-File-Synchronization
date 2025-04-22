use std::{fs, io::Read};

use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::{fileDifs, fileFilter};

pub async fn write_full_file_to_connection(filepath: &str, stream: &mut TcpStream){
    println!("Writing to connections:");
    // No idea why this is warning me that the mut is unused because removing it makes it an error
    let mut f = match fs::File::open(filepath){
        Ok(x) => x,
        Err(_) => {
            // Skip me file not found
            return;
        }
    };

    let base_path = fileFilter::Filter::get_instance().get_base_dir();
    let relative_path = filepath.replace(base_path, ""); // Removing base path from the file path to get relative path
    let mut relative_path_bytes = relative_path.into_bytes();
    relative_path_bytes.push(0b0000);
    // Relative path now ends with a null byte which will never be allowed in a file name
    // This is one of two characters that are completely illegal
    relative_path_bytes.push(1u8);
    // Push identifier for the full file send
    
    match stream.write(relative_path_bytes.as_slice()).await{
        Ok(x) => x,
        Err(x) => {
            // println!("Error on writing relative path: {} on connection address: {}",x, address);
            return;
        }
    };
    
    let mut buf = [0u8; 1024];
    let file_length = f.metadata().unwrap().len();
    println!("File length: {}", file_length);
    let _ = stream.write(&file_length.to_le_bytes()).await.expect("Failed to write file length");
    // let mut reader = BufReader::new(f);
    let mut total_written = 0;
    loop {
        let num_bytes = f.read(&mut buf).expect(format!("Failed to read file: {}", filepath).as_str());
        if num_bytes == 0 {
            break;
        }
        total_written += num_bytes;
        match stream.write_all(&buf[..num_bytes]).await{
            Ok(x) => x,
            Err(_) => {
                // println!("Failed to write to connection: {} while writing file data. Error: {}", address, x);
                return;
            }
        };
        // if num_bytes == 0 {
        //     break;
        // }
    }
    println!("Finished writing, bytes written: {}", total_written);
}
pub async fn write_delta_to_connection(delta: &fileDifs::Delta, filepath: &str, stream: &mut TcpStream){
    // let mut addr = Connections::get_instance().await.addresses.write().unwrap();
    // for (address, connection) in addr.iter_mut() {
    println!("writing a delta");
    // println!("stream.read complete");
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
    let _ = stream.write(&relative_path_bytes).await;
    // relative_path_bytes.push(delta.start_index.to_le_bytes());
    // stream.flush();

        
                        
    // }
}

pub async fn write_deletion_to_connection(filepath: &str, stream: &mut TcpStream){
    println!("Deleting a file");
    let base_path = fileFilter::Filter::get_instance().get_base_dir();
    let relative_path = filepath.replace(base_path, ""); // Removing base path from the file path to get relative path
    println!("Sending delete for: {}", relative_path);
    let mut relative_path_bytes = relative_path.into_bytes();
    relative_path_bytes.push(0b0000);
    relative_path_bytes.push(4u8);
    let _ = stream.write(&relative_path_bytes).await;

}
pub async fn write_move_to_connection(filepath_old: &str, filepath_new: &str, stream: &mut TcpStream){
    println!("moving a file");
    let base_path = fileFilter::Filter::get_instance().get_base_dir();
    let relative_path_old = filepath_old.replace(base_path, ""); // Removing base path from the file path to get relative path
    let relative_path_new = filepath_new.replace(base_path, "");
    let mut relative_path_bytes = relative_path_old.into_bytes();
    relative_path_bytes.push(0b0000);
    relative_path_bytes.push(3u8);
    stream.write(&relative_path_bytes).await.expect("Failed to write to stream, A");
    let mut new_path_bytes = relative_path_new.into_bytes();
    new_path_bytes.push(0b0000);
    stream.write(&new_path_bytes).await.expect("Failed to write to stream?");


}
pub async fn write_create_dir_to_connection(dirpath: &str, stream: &mut TcpStream){
    println!("creating a dir");
    let base_path = fileFilter::Filter::get_instance().get_base_dir();
    let relative_path_old = dirpath.replace(base_path, ""); // Removing base path from the file path to get relative path
    // let relative_path_new = filepath_new.replace(base_path, "");
    let mut relative_path_bytes = relative_path_old.into_bytes();
    relative_path_bytes.push(0b0000);
    relative_path_bytes.push(6u8);
    let _ = stream.write(&relative_path_bytes).await;
}
pub async fn write_create_file_to_connection(filepath: &str, stream: &mut TcpStream){
    println!("Creating a file");
    let base_path = fileFilter::Filter::get_instance().get_base_dir();
    let relative_path_old = filepath.replace(base_path, ""); // Removing base path from the file path to get relative path
    // let relative_path_new = filepath_new.replace(base_path, "");
    let mut relative_path_bytes = relative_path_old.into_bytes();
    relative_path_bytes.push(0b0000);
    relative_path_bytes.push(5u8);
    let _ = stream.write(&relative_path_bytes).await;
}