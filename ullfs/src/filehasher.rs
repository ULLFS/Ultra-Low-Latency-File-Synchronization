use std::{fs, io::BufReader};

use anyhow::Error;
use bytes::Bytes;
use serde_json::Value;

pub async fn hash_check(filepath: &str) -> Result<Bytes,Error>{
    // using a result so I can use the ? operator

    let f = fs::File::open(filepath)?;
    let r = BufReader::new(f);    
    
    return Ok(Bytes::new());
}