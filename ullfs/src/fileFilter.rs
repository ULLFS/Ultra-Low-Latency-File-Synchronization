use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::{fs, io::BufReader};
use std::sync::OnceLock;
use serde_json::Value;

static INSTANCE: OnceLock<Filter> = OnceLock::new();

// Creating a singleton for fileFiltering.
// This way we store all our data without 
// This isn't the correct rust way of doing this but I don't know the correct method
pub struct Filter{
    ignore: Gitignore,
    baseDir: String,


    
}
impl Filter{
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
        let watch_dir : String = match &conf["watch_dir"].as_str() {
            None => {
                panic!("Error: watch_dir was not a string in config.json");
            }
            Some(x) => x.to_string(),
        };
        
        
        let ignore_rules: Vec<Value> = match &conf["ignore"].as_array(){
            None => {
                panic!("Error: ignore was not an array of values in config.json");
            }
            Some(x) => x.to_vec(),
        };
        let mut ignoreBuilder: GitignoreBuilder = GitignoreBuilder::new(watch_dir.clone());
        for val in ignore_rules{
            let valStr: String = match val.as_str(){
                None => {
                    panic!("Error: value in ignore was not a string in config.json");
                }
                Some(x) => x.to_string(),
            };
            match ignoreBuilder.add_line(None, valStr.as_str()){
                Ok(_) => (),
                Err(e) => {
                    panic!("Error: Mistake in ignore. Treat each element of the array as gitignore line. {}", e);
                }
            }
        }
        let ignorer = match ignoreBuilder.build(){
            Ok(x) => x,
            Err(e) =>{
                panic!("Error: Something when wrong with building the ignorer. Make sure the ignore config.json is correct. Every element should be treated as a gitignore line. {}", e);
            }
        };



        Filter {
            ignore: ignorer,
            baseDir: watch_dir,
        }
        
    }
    
    pub fn get_base_dir(&self) -> &str {
        &self.baseDir.as_str()
    }

    pub fn get_instance() -> &'static Filter{
        INSTANCE.get_or_init(|| Filter::new())
    }

    pub fn should_filter(&self, path: &str) -> bool{
        self.ignore.matched(path, false).is_ignore();
        false
    }
    
    // Getter for client_port
    pub fn get_client_port(&self) -> &str {
        &self.client_port
    }

    // Getter for baseDir
    pub fn get_base_dir(&self) -> &str {
        &self.base_dir
    }
    
    
}

