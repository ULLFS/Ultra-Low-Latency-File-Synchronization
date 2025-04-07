// use difference::{self, Changeset, Difference};
// use fossil_delta::{delta, deltainv};
// use librsync::Delta;

// use core::slice::SlicePattern;
use std::{env::temp_dir, error::Error, fs::{self, File}, io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write}};
use tokio::net::{TcpStream, TcpListener};
// use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};
use xxhash_rust::xxh3::xxh3_64;

#[derive(PartialEq, Eq)]
enum State {
    Filepath,
    Flag,
    FileLength,
    FileData,
    FileDeltaStart,
    FileDeltaEnd,
    FileDeltaData,
    FileDeltaDataLength,
    FileDeltaHash,
    FileDeltaDataToss,
    // FileDeltaHashEnd,
}

fn ask_for_file(file: &String, _stream: &TcpStream){
    println!("Got the wrong hash for the file: {}", file);
}

fn filepath_state(b: u8, state: &mut State, filepath: &mut String, curbyte: usize, total_bytes: usize) {
    if b == 0b0000 {
        println!("Changing to flag state, {}, {}", curbyte, total_bytes);
    
        *state = State::Flag;
    }
    else {
        filepath.push(char::from(b));
        println!("{}", filepath);
    }
}

fn flag_state(b: u8, state: &mut State, writer: &mut Option<BufWriter<File>>, reader: &mut Option<BufReader<File>>, cur_index: &mut u64, curbyte: usize, total_bytes: usize, file_path: &String, save_path : &str){
    println!("In flag state {}, {}", curbyte, total_bytes);
    match b {
        1u8 => {
            *cur_index = 0;
            println!("Changing to fileLength state");
            *state = State::FileLength;
        }
        2u8 => {
            *cur_index = 0;
            println!("Changing to file delta state");
            let local_path = format!("{}/{}", save_path, file_path);
            println!("File: {}", local_path);
            let f = fs::File::open(&local_path).expect("Failed to open the file for deltas");
            *writer = Some(BufWriter::new(f));
            let f = fs::File::open(local_path).expect("Failed to open the file for deltas");
            
            *reader = Some(BufReader::new(f));
            // file = Some(fs::File::open(filepath).expect("Failed to open the file"));
            
            *state = State::FileDeltaStart;
        }
        _ => {
            println!("Got bad data: {}", b);
        }
    }
}

fn file_length_state(b: u8, state: &mut State, writer: &mut Option<BufWriter<File>>, cur_index: &mut u64, file_path: &String, save_path : &str, filelength: &mut u64){
    // println!("{}",2u64.pow(cur_index as u32));
    *filelength += (b as u64) << 8 * *cur_index;
    *cur_index += 1;
    println!("File length: {}", filelength);
    if *cur_index >= 8{
        println!("Changing to filedata state");
        *state = State::FileData;
        *cur_index = 0;
        let local_path = format!("{}/{}", save_path, file_path);
        // if !fs::exists(&local_path).expect("Why would this ever error, error on fs exists") {
        //     fs::File::create(&local_path).expect(format!("Failed to create file that didn't exist {}", local_path).as_str());
        // }
        let f = fs::File::create(&local_path).expect(format!("failed to open file: {}", local_path).as_str());
        *writer = Some(BufWriter::new(f));
    }
}

fn pull_u64(b: u8, state: &mut State, data: &mut u64, cur_index: &mut u64, next_state: State) {
    println!("{}",b);
    let shift_amount;
    // assert!(8 * *cur_index < 63);
    shift_amount = (b as u64) << 8 * *cur_index;
    println!("Shift amount: {}", shift_amount);
    *data += shift_amount;
    *cur_index += 1;
    println!("{}\nIndex: {}",data, cur_index);
    
    if *cur_index >= 8 {
        *cur_index = 0;
        *state = next_state;
    }
}

pub async fn processing(stream: &TcpStream, save_path : &str) {
    println!("Accepted connection");
    // let mut stream = a.0;
    let mut state = State::Filepath;
    let mut filepath: String = "".to_string();
    let mut filelength: u64 = 0;
    let mut cur_index: u64 = 0;
    let mut writer: Option<BufWriter<File>> = None;
    let mut reader: Option<BufReader<File>> = None;
    let mut hash: u64 = 0;
    let mut start_delta: u64 = 0;
    let mut end_delta: u64 = 0;
    
    println!("Listening for data");
    loop {
        let mut buf = [0; 1024];
        let size = match stream.try_read(&mut buf){
            Ok(x) => x,
            Err(x) => {
                if x.kind() != std::io::ErrorKind::WouldBlock{
                    // println!();
                    panic!("Non blocking error: {}", x)
                }
                continue;
            }
        };
        if size != 0 {
            println!("Got data");
            for i in 0..size + 1{
                let b = buf[i];
                match state {
                    State::Filepath => {
                        filepath_state(b, &mut state, &mut filepath, i, size);
                    }
                    State::Flag => {
                        flag_state(b, &mut state, &mut writer, &mut reader, &mut cur_index, i, size, &filepath, save_path);
                    }
                    State::FileLength => {
                        file_length_state(b, &mut state, &mut writer, &mut cur_index, &filepath, save_path, &mut filelength);
                    }
                    State::FileData => {
                        // This one can't be a function easily because we don't know the size of the u8 array
                        // println!("Writing bytes {} to {}", i, size);
                        let f = writer.as_mut().unwrap();
                        // Writing all the rest of the bytes at once
                        let length = size as u64 - i as u64;
                        let mut end = size;
                        if filelength < cur_index + length {
                            end = length as usize - cur_index as usize;
                        }
                        match f.write(&buf[i..end]) {
                            Ok(_) => {},
                            Err(x) => {
                                panic!("Failed to write for reason: {}", x);
                            }
                        }
                        
                        cur_index += size as u64 - i as u64;
                        if cur_index >= filelength {
                            cur_index = 0;
                            println!("Finished reading file {}", filepath);
                            filepath = "".to_string();
                            state = State::Filepath;
                            filelength = 0;
                            
                            f.flush().expect("Failed to flush writer");
                        } else {
                            println!("{}: {}", cur_index, filelength);
                        }
                        
                        break;
                        
                        
                    }
                    State::FileDeltaHash => {
                        pull_u64(b, &mut state, &mut hash, &mut cur_index, State::FileDeltaData);  
                        if state == State::FileDeltaData {
                            // We have gotten the hash and now we compare it to the hash of the file we are testing against
                            let mut f;
                            if !fs::exists(save_path.to_string() + filepath.as_str()).expect("Failed to see if file exists") {
                                // If the file doesn't exist we don't even have to begin checking the hash
                                println!("File didn't exist");
                                ask_for_file(&filepath, &stream);
                                state = State::FileDeltaDataToss;
                                break;
                            }
                            f = fs::File::open(save_path.to_string() + filepath.as_str()).expect("Failed to open file");
                            let mut hash_buf: Vec<u8> = Vec::new();
                            f.read_to_end(&mut hash_buf).expect("Failed to read to end of file");
                            println!("{}", hash_buf.len());
                            let x_hash = xxh3_64(&hash_buf);
                            if x_hash != hash {
                                // We got the wrong hash, ask for the full file
                                ask_for_file(&filepath, &stream);
                                println!("Wrong hash: {}, correct hash: {}", hash, x_hash);
                                hash = 0;
                                
                                // let mut f = fs::File::create(DELTA_PATH.to_string() + )
                                state = State::FileDeltaDataToss;
                            } else {
                                hash = 0;
                                let write_file = fs::File::create(format!("{}{}.temp", save_path, filepath)).expect("Failed to create writable file");
                                let mut w = BufWriter::new(write_file);
                                // writer = Some(BufWriter::new(f));
                                // Dont have to set the state again, the pull u64 got that for us
                                let r = reader.as_mut().unwrap();
                                // Creating fixed size array with a box to read the specific bytes we needed
                                let mut buffer: Box<[u8]> = vec![0; start_delta as usize - 1].into_boxed_slice();
                                r.read(&mut buffer).expect("Failed to read");
                                let seek_num;
                                if start_delta > end_delta {
                                    seek_num = start_delta - 1;
                                } else {
                                    seek_num = end_delta;
                                }
                                r.seek(SeekFrom::Start(seek_num)).expect("Failed to seek"); // Seek to the next point we would be reading
                                
                                
                                // Seek to the end of the delta in order to skip over any previous things
                                w.write(&buffer).expect("Failed to write");
                                // We have now written to the start of the buffer
                                writer = Some(w);
                            }
                            
                        } 
                    }
                    State::FileDeltaStart => {
                        // let f = writer.as_mut().unwrap();
                        // file_delta_start_state(b, &mut start_delta, &mut cur_index);
                        print!("Starting value: ");
                        pull_u64(b, &mut state, &mut start_delta, &mut cur_index, State::FileDeltaEnd);
                        
                        
                    }
                    State::FileDeltaDataLength => {
                        print!("Length value: ");
                        
                        pull_u64(b, &mut state, &mut filelength, &mut cur_index, State::FileDeltaHash);
                        // println!("filelength: {}", filelength);
                    }
                    State::FileDeltaEnd => {
                        print!("End  value: ");
                        
                        pull_u64(b, &mut state, &mut end_delta, &mut cur_index, State::FileDeltaDataLength);
                    }
                    State::FileDeltaData => {
                        // This one can't be a function easily because we don't know the size of the u8 array
                        // println!("Writing bytes {} to {}", i, size);
                        let f = writer.as_mut().unwrap();
                        // let read = reader.as_mut().unwrap();
                        // Writing all the rest of the bytes at once
                        let length = size as u64 - i as u64;
                        let mut end = size;
                        if filelength < cur_index + length {
                            end = length as usize - cur_index as usize;
                        }
                        
                        match f.write(&buf[i..end]) {
                            Ok(_) => {},
                            Err(x) => {
                                panic!("Failed to write for reason: {}", x);
                            }
                        }
                        
                        cur_index += size as u64 - i as u64;
                        if cur_index >= filelength {
                            // Get the length of the reader file
                            println!("{}", filepath);
                            // let reader_size = reader.unwrap().into_inner().metadata().unwrap().len();
                            let fsize = fs::File::open(format!("{}{}", save_path, filepath)).unwrap().metadata().unwrap().len();
                            println!("fsize: {}", fsize);
                            println!("End delta: {}", end_delta);
                            
                            let end;
                            if end_delta > start_delta {
                                end = fsize - end_delta;
                            } else {
                                end = fsize - start_delta + 1;
                            }
                            println!("data size: {}", fsize - (fsize - end));
                            let mut buffer: Box<[u8]> = vec![0; end as usize].into_boxed_slice();
                            match reader.as_mut() {
                                Some(x) => {
                                    x.read(&mut buffer).expect("Failed to read to buffer");
                                    f.write(&buffer).expect("Failed to write to buffer");
                                }
                                None => {
                                    panic!("Reader was not initialized in deltas none");
                                }
                            };
                            f.flush().expect("Failed to flush writer");
                            fs::rename(format!("{}{}.temp", save_path, filepath), format!("{}{}", save_path, filepath)).expect("Failed to write to file");
                            cur_index = 0;
                            println!("Finished delta file {}", filepath);
                            filepath = "".to_string();
                            state = State::Filepath;
                            filelength = 0;
                            end_delta = 0;
                            start_delta = 0;
                            
                            
                        } else {
                            println!("{}: {}", cur_index, filelength);
                        }
                        
                        break;
                    }
                    State::FileDeltaDataToss => {
                        // This is likely not the most efficient way to do this but it is the easiest
                        cur_index += 1;
                        println!("Index in data toss: {} / {}", cur_index, filelength);
                        if cur_index > filelength {
                            println!("Got out of filedeltadata");
                            state = State::Filepath;
                            cur_index = 0;
                            filepath = String::new();
                        }
                    }
                }    
            }
            
            
        }
        
    }
    
}