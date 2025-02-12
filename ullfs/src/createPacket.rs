use std::{fs::{self, File}, io::Read};
// #[ignore(unused_parens)];
#[allow(unused_parens)]
pub struct FullFileData{
    path : String,
    timestamp : String,
    data : [u8]
}

pub fn create_full_file_packet(local_filepath : &str, full_filepath : &str, mtu : i32, mut buf : Vec<u8>){
    
    let mut file: File = File::open(full_filepath).expect(format!("Failed to open {}", full_filepath).as_str());
    
    buf.push(0b0000); // First byte should determine what type of packet
    /// 0b0000 refers to full file send packet
    // two bytes to store length of path:
    // Max path size is 4096, so we only have to use a u16 to represent that data
    let pathsize = local_filepath.len();
    if pathsize > 4096{
        panic!("Filepath too long (over 4096 bytes), how did this happen?");
    }
    let pathsize_u16 = pathsize as u16;
    let pathsize_first_byte = (pathsize_u16 >> 8) as u8;
    let pathsize_second_byte = (pathsize_u16 & 0xFF) as u8;
    buf.push(pathsize_first_byte);
    buf.push(pathsize_second_byte);
    
    
    for byte in local_filepath.as_bytes(){    
        buf.push(*byte);
    }
    let l: usize = file.metadata().unwrap().len() as usize;

    // let buf: &mut Vec<u8> = vec![0; l];

    // fs::File::read(&mut file, buf);
    
}
// pub fn read_full_file_packet(data : &[u8], size: usize) -> full_file_data{
    
// }