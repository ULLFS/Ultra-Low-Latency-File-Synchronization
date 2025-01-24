// use std::{fs::{self, create_dir_all}, hash::Hash, io::{BufReader, Read}};
// use env_logger::fmt::Timestamp;
// use xxhash;
// use anyhow::Error;
// use bytes::Bytes;
// use serde_json::Value;
// use std::collections::HashMap;
// static INSTANCE: OnceLock<FileData> = OnceLock::new();
// pub struct File {
//     data: Bytes,
//     startTime: Timestamp,
// }
// pub struct FileData{
//     fileMap: HashMap<String, File>,
//     fileStoreTime: u32,
//     maxTotalSize: u32,
//     curTotalSize: u32
// }
// impl FileData {
//     fn new() -> Self {
//         let conf_file : fs::File = match fs::File::open("./config.json"){
//             Ok(x) => x,
//             Err(e) => {
                
//                 panic!("Error: config.json missing or destroyed.\n{}", e)
//             }
//         };
//         let reader = BufReader::new(conf_file);
//         let conf : Value = match serde_json::from_reader(reader){
//             Ok(x) => x,
//             Err(e) => {
//                 panic!("Error: config.json structure damaged.\n{}", e);
//             }
//         }; 
//         let fileStoreTime: u32 = match conf["file_store_time_seconds"]{
//             Ok(x) =>x,
//             Err(e) => {
//                 panic!("Error: file_store_time_seconds did not exist in config.json or was not an integer. {}", e);
//             }
//         };
//         let maxTotalSize: u32 = match conf["max_total_size_gb"]{
//             Ok(x) => x,
//             Err(e) => {
//                 panic!("Error: max_total_size_gb did not exist in config.json or was not an integer. {}", e);
//             }
//         };

//         let map: HashMap<String, File>;
//         FileData { 
//             fileMap: map,
//             fileStoreTime: fileStoreTime,
//             maxTotalSize: maxTotalSize,
//             curTotalSize: 0
//         }
//     }
//     pub fn get_instance() -> &'static Filter{
//         INSTANCE.get_or_init(|| Filter::new())
//     }
//     pub fn isFileHeld(&self, path: String) -> bool {
//         self.fileMap.contains_key(&path)
//         // self.hash_block_size
//     }
//     pub fn getFinalHashSize(&self) -> u64 {
//         self.final_hash_size
//     }
//     pub fn getHashStorePath(&self) -> String {
//         self.hash_store_path
//     }
// }
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