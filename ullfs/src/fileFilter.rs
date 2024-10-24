use std::{fs, io::BufReader, vec};

use serde_json::Value;
static mut ignore_hidden: bool = true;
fn set_ignore_hidden(b: bool) {
    unsafe {ignore_hidden = b};
}
fn get_ignore_hidden() -> bool {
    unsafe {ignore_hidden}
}
pub fn init(){
    let conf_file : fs::File = match fs::File::open("./config.json"){
        Ok(x) => x,
        Err(e) => {
            panic!("Error: config.json missing or destroyed.\n{}", e)
        }
    };
    // Convert to buffer for serde_json
    let reader = BufReader::new(conf_file);
    let conf : Value = match serde_json::from_reader(reader){
        Ok(x) => x,
        Err(e) => {
            panic!("Error: config.json structure damaged.\n{}", e);
        }
    };
    set_ignore_hidden(match conf["ignore_hidden_files"].as_bool(){
        Some(x) => x,
        None => {
            panic!("Error: ignore_hidden_files missing or not a bool in config.json");
        }
    });
}
pub fn filter(filepath: &str) -> bool{
    let f = filepath.split('/').collect::<Vec<&str>>();
    let num = f.len();
    let filename = f[num - 1];

    if filename.as_bytes()[0] == 46u8 && get_ignore_hidden() {
        println!("Looked at hidden file");
        return false;
    }


    true
}