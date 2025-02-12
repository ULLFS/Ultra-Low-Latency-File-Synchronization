pub struct hash {
    hash: u64
}
impl hash {
    pub fn new(data : u64) -> Self{
        hash{
            hash: data
        }
    }
}
pub fn hash_chunk(data : Vec<u8>)-> hash{
    
    
    let mut data_arr: Vec<u8> = Vec::new();
    for b in data {
        data_arr.push(b);
    }
    let h = xxhash_rust::xxh3::xxh3_64(&data_arr);
    return hash::new(h);
}
pub fn chunk_hash_file(path : &str) -> Vec<hash>{
       return Vec::new();
}