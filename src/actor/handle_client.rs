// use difference::{self, Changeset, Difference};
// use fossil_delta::{delta, deltainv};
// use librsync::Delta;

// use core::slice::SlicePattern;
use std::{env::temp_dir, error::Error, fs::{self, create_dir_all, File}, future::Future, io::{BufRead, BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write}, path::Path, pin::Pin, usize::MAX};
use steady_state::SteadyCommander;
use tokio::{io::{self, AsyncRead, AsyncReadExt}, net::{TcpListener, TcpStream}};
// use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};
use xxhash_rust::xxh3::xxh3_64;
const MAX_LENGTH_READ: u64 = 1024;

fn create_global_path(save_path : &str, path: &str) -> String {
    let full_path = format!("{}/{}", save_path, path);
    return full_path;
}

async fn full_file<R: AsyncRead + Unpin>(
    path: String,
    length: u64,
    reader: &mut R,
    save_path : &str
) -> io::Result<()> {
    // Your file handling logic
    // stream_data(&path, length, reader).await
    let full_path = create_global_path(save_path, &path);
    println!("Writing full file for: {}, length: {}", full_path, length);
    let mut cur_read = 0;
    let file_path = Path::new(&full_path);
    match file_path.parent() {
        Some(x) => {
            if !x.exists() {
                fs::create_dir_all(x).expect("Failed to create file directory");
            }
        }
        None => {}
    };
    let mut writer = fs::File::create(&full_path).expect("failed to create writer file");
    while cur_read < length {
        
        let read_amount = std::cmp::min(MAX_LENGTH_READ, length - cur_read);
        cur_read += read_amount;
        println!("Read amount: {}", read_amount);
        let mut arr: Box<[u8]> = vec![0; read_amount as usize].into_boxed_slice();
        reader.read_exact(&mut arr).await?;
        // Do the stuff here with arr
        writer.write(&arr).expect("Failed to write to writer");


    }
    Ok(())
}
async fn read_null_terminated_string<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<String> {
    let mut buf = Vec::new();
    let mut byte = [0u8];
    while reader.read_exact(&mut byte).await.is_ok() {
        if byte[0] == 0 {
            break;
        }
        buf.push(byte[0]);
    }
    String::from_utf8(buf).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
}
async fn delta_file<R: AsyncRead + Unpin>(
    path: &str,
    start: u64,
    end: u64,
    length: u64,
    hash: u64,
    reader: &mut R,
    save_path : &str
) -> io::Result<()> {
    let global_path = create_global_path(save_path, path);
    let path = Path::new(&global_path);
    println!(
        "[delta] {}: bytes {}-{} (len {}) hash={}",
        path.display(), start, end, length, hash
    );
    // let path_dir = path.parent();
    match path.parent() {
        Some(x) => {
            if !x.exists() {
                fs::create_dir_all(x).expect("Failed to create file directory");
            }
        }
        None => {}
    };
    let path_temp = global_path.to_string() + ".temp";
    let hash_correct = {
        let mut file_reader = fs::File::open(&path).expect("failed to open file");
        let mut sbuf = Vec::new();
        file_reader.read_to_end(&mut sbuf).expect("Failed to read file to hash");
        let hash_val = xxh3_64(&mut sbuf);
        hash_val == hash
    };
    let mut cur_read = 0;
    // let mut temp_writer = None;

    if hash_correct {
        println!("Hash was correct");
        let mut file_reader = fs::File::open(path).expect("failed to open file");

        // if we have the correct hash, read the file first up to start
        let seek_num;

        if start > end {
            seek_num = start - 1;
        } else {
            seek_num = end;     
        }
        // file_reader.seek(SeekFrom::Start(seek_num)).expect("failed to seek");
        let mut temp_writer = fs::File::create(&path_temp).expect("Failed to create file");
        println!("Created tempwriter and will be reading: {}", start - 1);
        let mut cur_read_file = 0;
        while cur_read_file < start - 1 {
            let read_amount = std::cmp::min(MAX_LENGTH_READ, start - 1 - cur_read_file);
            cur_read_file += read_amount;
            let mut arr: Box<[u8]> = vec![0; read_amount as usize].into_boxed_slice();
            file_reader.read_exact(&mut arr).expect("Failed to read from the reader");
            temp_writer.write(&arr).expect("Failed to write to temp file");
        }
        // Then read the data from the stream
        println!("Reading stream data for: {}", length);
        while cur_read < length {
            let read_amount = std::cmp::min(MAX_LENGTH_READ, length - cur_read);
            println!("Reading amount: {}", read_amount);
            cur_read += read_amount;
            let mut arr: Box<[u8]> = vec![0; read_amount as usize].into_boxed_slice();
            reader.read_exact(&mut arr).await?;
            // Do the stuff here with arr
            temp_writer.write(&arr).expect("Failed to write data");
            
        }
        // Then seek to the proper position and write the rest of the file
        file_reader.seek(SeekFrom::Start(seek_num)).expect("Failed to seek");
        let mut cur_read_file = seek_num;
        let file_length = file_reader.metadata().unwrap().len();
        while cur_read_file < file_length {
            println!("File length: {}, cur_read_file: {}", file_length, cur_read_file);
            let read_amount = std::cmp::min(MAX_LENGTH_READ, file_length - cur_read_file);
            cur_read_file += read_amount;
            let mut arr: Box<[u8]> = vec![0; read_amount as usize].into_boxed_slice();
            file_reader.read_exact(&mut arr).expect("Failed to read from the reader");
            temp_writer.write(&arr).expect("Failed to write to temp file");
        }
        // temp_writer = writer;/
        // Only let us write if it isn't none
        // Let's write to the start value:
        fs::rename(path_temp, global_path).expect("Failed to rename file");
        println!("Finished with the delta!");

    } else {
        // seek_num = 0;
        println!("hash incorrect, reading excess data for {} bytes", length);
        while cur_read < length {
            let read_amount = std::cmp::min(MAX_LENGTH_READ, length - cur_read);
            cur_read += read_amount;
            let mut arr: Box<[u8]> = vec![0; read_amount as usize].into_boxed_slice();
            reader.read_exact(&mut arr).await?;
            // Do the stuff here with arr
            // In this case we just skip all of it
            // temp_writer.write(&arr).expect("Failed to write data");
            
        }
        println!("Hash incorrect!");
    }
    
    
    
    // stream_data(&path, length, reader).await
    Ok(())
}

pub async fn processing<C: SteadyCommander>(mut stream: TcpStream, save_path : &str, cmd: &mut C) {
    println!("Accepted connection");
    // let mut stream = a.0;
    // let handler = MyHandler;
    loop {
        match decode_stream(&mut stream, save_path).await {
            Ok(_) => {}
            Err(_) =>{
                break;
            }
        }

    }
    
}
pub async fn decode_stream<R: AsyncRead + Unpin + Send>(
    mut reader: R,
    save_path: &str
) -> io::Result<()> {
    loop {
        let path = read_null_terminated_string(&mut reader).await?;
        let flag = reader.read_u8().await?;

        match flag {
            1 => {
                let length = reader.read_u64_le().await?;
                full_file(path, length, &mut reader, save_path).await?;
            }
            2 => {
                let start = reader.read_u64_le().await?;
                let end = reader.read_u64_le().await?;
                let length = reader.read_u64_le().await?;
                let hash = reader.read_u64_le().await?;
                delta_file(&path, start, end, length, hash, &mut reader, save_path).await?;
            }
            other => {
                return Err(io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Unknown flag: {}", other),
                ));
            }
        }

        // After processing one file, continue to the next file (if any)
    }
}

