use std::{fs::{self}, io::{BufReader, Read}, ops::Add};
// use bytes::Bytes;
use serde_json::Value;
use xxhash_rust::xxh3::{xxh3_64, Xxh3};
use std::time::SystemTime;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
static INSTANCE: OnceLock<FileData> = OnceLock::new();
pub struct File {
    data: Vec<u8>,
    time_remaining: u32,
}
impl File {
    fn new(data: Vec<u8>, time_remaining : u32) -> Self {
        File {
            data: data,
            time_remaining: time_remaining
        }
    }
}
pub struct Delta {
    pub start_index: u64,
    pub end_index: u64,
    pub data: Vec<u8>,
    pub old_hash: u64
}
impl Delta {
    pub fn new(start_index: u64, end_index: u64, data: Vec<u8>, old_hash: u64) -> Self{
        Delta {
            start_index: start_index,
            end_index: end_index,
            data: data,
            old_hash: old_hash
        }
    }
}
pub struct FileData{
    file_map: RwLock<HashMap<String, File>>,
    file_store_time: u32,
    max_total_size: u64,
    cur_total_size: RwLock<u64>
}
impl FileData {
    fn new<'a>() -> Self {
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
        let file_store_time = match conf["file_store_time_minutes"].as_u64(){
            Some(x) =>x as u32,
            None => {
                panic!("Error: file_store_time_minutes did not exist in config.json or was not an integer.");
            }
        };
        let max_total_size = match conf["max_total_size_mb"].as_u64(){
            Some(x) => x,
            None => {
                panic!("Error: max_total_size_gb did not exist in config.json or was not an integer.");
            }
        };

        // let map: HashMap<String, File> = );
        let map: HashMap<String, File> = HashMap::new();
        FileData { 
            file_map: RwLock::new(map),
            file_store_time: file_store_time,
            max_total_size: max_total_size,
            cur_total_size: RwLock::new(0)
        }
    }
    pub fn get_instance() -> &'static FileData{
        INSTANCE.get_or_init(|| FileData::new())
    }
    // pub fn isFileHeld(&self, path: String) -> bool {
    //     self.fileMap.contains_key(&path)
    //     // self.hash_block_size
    // }
    // async fn timeout(){
        
    // }
    pub fn add_file(&self, filepath: String) {
        let mut file_map_lock = self.file_map.write().unwrap();
        let mut file = match fs::File::open(&filepath) {
            Ok(x) => x,
            Err(_) => return
        };
        let mut size_lock = self.cur_total_size.write().unwrap();

        if file.metadata().unwrap().len() <= (self.max_total_size - *size_lock) {
            // size_lock += file.metadata().unwrap().len();
            size_lock.add(file.metadata().unwrap().len());

        } else {
            // Not enough space left
            return;
        }
        let mut file_data: Vec<u8> = Vec::new();
        file.read_to_end(&mut file_data);
        let f = File::new(file_data, self.file_store_time);
        file_map_lock.insert(filepath, f);
    }
    pub fn clean_ram(&self, minutes_passed: u32) -> bool{
        let mut files = match self.file_map.try_write() {
            Ok(mut files) => files,
            Err(e) => {
                println!("Failed to clean RAM, error: {}", e);
                return false;
            }
        };
        
        files.retain(|_, file| {
            if file.time_remaining >= minutes_passed {
                file.time_remaining -= minutes_passed;
                true
            } else {
                println!("Deleting a file");
                false
            }
        });
        
        true
    }
    pub fn contains_file(&self, path: &str) -> bool {
        let map = self.file_map.read().unwrap();
        map.contains_key(path)
    }
    pub fn remove_file(&self, path: &str) {
        let mut map = self.file_map.write().unwrap();
        map.remove(path);
    }
    pub fn get_file_delta(&self, path : &str) -> Option<Delta,>{
        println!("{}", path);

        let mut f = match fs::File::open(&path){
            Ok(x) => x,
            Err(e) => {
                return None;
            }
        };
        let mut buf : Vec<u8> = Vec::new();
        let r = fs::File::read_to_end(&mut f, &mut buf);
        // println!("A");
        let instance = self::FileData::get_instance();
        // println!("A");
        // RwLock essentially acts as a semaphore.
        // As soon as map falls out, the lock is opened.
        // This will wait for its turn
        let mut map = instance.file_map.write().unwrap();
        // println!("A");
        
        let file_data = match map.get(path){
            Some(x) => x,
            None => {
                // println!("B");
                // let mut write_map = instance.file_map.write().unwrap();
                // println!("B");
                
                let data_clone = buf.clone();
                // println!("B");
                
                let f: File = File::new(buf, self.file_store_time);
                // Create a static str:
                // let path_static: &'static str = Box::leak(Box::new(path.to_string()));
                // let f_static : &'static mut File = Box::leak(Box::new(f));
                map.insert(path.to_string(), f);
                return Some(Delta::new(0,0, data_clone, 0));
                
            }
        };
        let output_data = get_delta(&file_data.data, &buf);
        
        let f = File::new(buf, self.file_store_time);
        *map.get_mut(path).unwrap() = f;
        return Some(output_data);
    }
}
// pub async fn hash_check(filepath: &str){
//     // using a result so I can use the ? operator

//     let f = match fs::File::open(filepath){
//         Ok(x) => x,
//         Err(_) => {
//             return;
//         }
//     };
//     let r = BufReader::new(f);
    
    
// }
pub fn get_delta(old: &Vec<u8>, new: &Vec<u8>) -> Delta {
    let mut start_index = 0;
    
    // Find the first index where the vectors differ
    while start_index < old.len() && start_index < new.len() && old[start_index] == new[start_index] {
        start_index += 1;
    }

    // Find the last index where the vectors differ
    let mut old_end = old.len();
    let mut new_end = new.len();
    while old_end > start_index && new_end > start_index && old[old_end - 1] == new[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }
    
    // Determine the end index for deletion in `a`
    let end_index = old_end;
    
    // Determine the data to be added (from `b` starting at `start_index` to `b_end`)
    let data_to_add = if new_end > start_index {
        new[start_index..new_end].to_vec()
    } else {
        Vec::new()
    };
    let old_data = xxh3_64(old);
    // REMINDER: When reading these deltas later, if end_index < start_index delete no bytes.
    // These are also 1 indexed in order to prevent errors when start index is 0 and we are only adding
    return Delta::new((start_index + 1) as u64, end_index as u64, data_to_add, old_data)
    // (start_index, end_index - 1, data_to_add)
    // If start_index <= end_index, delete all bytes between start and end including both
}
// pub async fn create_hash(filepath: String, basePath: String){
//     let hd: HashData = HashData::get_instance();
//     let f = match fs::File::open(filepath.clone()){
//         Ok(x) => x,
//         Err(_) => {
//             return;
//         }
//     };
//     let mut r = BufReader::new(f);
//     let bufSize = hd.getHashBlockSize();
//     let mut buf: [u8] = [0; bufSize];
//     let bytes = r.bytes();

//     for byteRes in bytes {
//         let byte = match byteRes {
//             Ok(x) => x,
//             Err(_) => {
//                 break;
//             }
//         };
//     }
//     pub fn get_instance() -> &'static Filter{
//         INSTANCE.get_or_init(|| Filter::new())
//     }

// }