use ignore::gitignore::{/* Gitignore, */ GitignoreBuilder};
use std::{fs, io::BufReader};
use std::sync::OnceLock;
use serde_json::Value;

static INSTANCE: OnceLock<Filter> = OnceLock::new();

// Creating a singleton for file filtering.
// This way we store all our data without recreating the structure multiple times.
pub struct Filter {
    /* ignore: Gitignore, */
    base_dir: String,
    server_ip: String,
    server_port: String,
}

impl Filter {
    // Private constructor for the singleton
    fn new() -> Self {
        let conf_file: fs::File = match fs::File::open("./config.json") {
            Ok(x) => x,
            Err(e) => panic!("Error: config.json missing or corrupted.\n{}", e),
        };

        let reader = BufReader::new(conf_file);
        let conf: Value = match serde_json::from_reader(reader) {
            Ok(x) => x,
            Err(e) => panic!("Error: config.json structure damaged.\n{}", e),
        };

        let watch_dir: String = match conf["watch_dir"].as_str() {
            Some(x) => x.to_string(),
            None => panic!("Error: watch_dir was not a string in config.json"),
        };

        let f_server_ip: String = match conf["server_ip"].as_str() {
            Some(x) => x.to_string(),
            None => panic!("Error: server_ip was not a string in config.json"),
        };

        let f_server_port: String = match conf["server_port"].as_str() {
            Some(x) => x.to_string(),
            None => panic!("Error: server_port was not a string in config.json"),
        };

        let ignore_rules: Vec<Value> = match conf["ignore"].as_array() {
            Some(x) => x.to_vec(),
            None => panic!("Error: ignore was not an array in config.json"),
        };

        let mut ignore_builder = GitignoreBuilder::new(&watch_dir);
        for val in ignore_rules {
            let val_str: String = match val.as_str() {
                Some(x) => x.to_string(),
                None => panic!("Error: value in ignore was not a string in config.json"),
            };
            if let Err(e) = ignore_builder.add_line(None, &val_str) {
                panic!("Error: Invalid ignore rule. Each element should be a valid gitignore pattern. {}", e);
            }
        }

        /* let ignorer = match ignore_builder.build() {
            Ok(x) => x,
            Err(e) => panic!("Error: Failed to build the ignorer. Check the ignore rules in config.json. {}", e),
        }; */

        Filter {
            /* ignore: ignorer, */
            base_dir: watch_dir,
            server_ip: f_server_ip,
            server_port: f_server_port,
        }
    }

    // Singleton accessor
    pub fn get_instance() -> &'static Filter {
        INSTANCE.get_or_init(|| Filter::new())
    }

    // Getter for dns_web_address
    pub fn get_server_ip(&self) -> &str {
        &self.server_ip
    }

    // Getter for client_port
    pub fn get_server_port(&self) -> &str {
        &self.server_port
    }

    // Getter for baseDir
    pub fn get_base_dir(&self) -> &str {
        &self.base_dir
    }

    // Determines if a file should be filtered
    /* pub fn should_filter(&self, path: String) -> bool {
        self.ignore.matched(&path, false).is_ignore()
    } */
}
