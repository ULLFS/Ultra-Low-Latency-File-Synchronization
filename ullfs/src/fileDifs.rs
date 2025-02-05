use std::{fs::{self}, io::{BufReader, Read}};
// use bytes::Bytes;
use serde_json::Value;
use std::time::SystemTime;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
static INSTANCE: OnceLock<FileData> = OnceLock::new();
pub struct File {
    data: Vec<u8>,
    start_time: SystemTime,
}
impl File {
    fn new(data: Vec<u8>, start_time : SystemTime) -> Self {
        File {
            data: data,
            start_time: start_time
        }
    }
}
pub struct FileData{
    file_map: RwLock<HashMap<&'static str, &'static File>>,
    file_store_time: u32,
    max_total_size: u32,
    cur_total_size: u32
}
impl FileData {
    fn new() -> Self {
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
        let file_store_time: u32 = match conf["file_store_time_seconds"].as_u64(){
            Some(x) =>x as u32,
            None => {
                panic!("Error: file_store_time_seconds did not exist in config.json or was not an integer.");
            }
        };
        let max_total_size: u32 = match conf["max_total_size_mb"].as_u64(){
            Some(x) => x as u32,
            None => {
                panic!("Error: max_total_size_gb did not exist in config.json or was not an integer.");
            }
        };

        // let map: HashMap<String, File> = );
        let map: HashMap<&'static str, &'static File> = HashMap::new();
        FileData { 
            file_map: RwLock::new(map),
            file_store_time: file_store_time,
            max_total_size: max_total_size,
            cur_total_size: 0
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
    pub fn get_file_delta(&self, path : &str) -> (usize, usize, Vec<u8>){
        println!("{}", path);
        let mut f = fs::File::open(&path).expect(format!("File not found {}", path).as_str());
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
                
                let f: File = File::new(buf, SystemTime::now());
                // Create a static str:
                let path_static: &'static str = Box::leak(Box::new(path.to_string()));
                let f_static : &'static File = Box::leak(Box::new(f));
                map.insert(path_static, f_static);
                return (0, 0, data_clone);
            }
        };
        let output_data = get_delta(&file_data.data, &buf);
        
        let f = File::new(buf, SystemTime::now());
        *map.get_mut(path).unwrap() = Box::leak(Box::new(f));
        return output_data;
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
pub fn get_delta(a: &Vec<u8>, b: &Vec<u8>) -> (usize, usize, Vec<u8>) {
    let mut start_index = 0;
    
    // Find the first index where the vectors differ
    while start_index < a.len() && start_index < b.len() && a[start_index] == b[start_index] {
        start_index += 1;
    }

    // Find the last index where the vectors differ
    let mut a_end = a.len();
    let mut b_end = b.len();
    while a_end > start_index && b_end > start_index && a[a_end - 1] == b[b_end - 1] {
        a_end -= 1;
        b_end -= 1;
    }
    
    // Determine the end index for deletion in `a`
    let end_index = a_end;
    
    // Determine the data to be added (from `b` starting at `start_index` to `b_end`)
    let data_to_add = if b_end > start_index {
        b[start_index..b_end].to_vec()
    } else {
        Vec::new()
    };
    // REMINDER: When reading these deltas later, if end_index < start_index delete no bytes.
    (start_index, end_index - 1, data_to_add)
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