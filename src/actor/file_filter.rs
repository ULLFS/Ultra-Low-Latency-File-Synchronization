use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::{fs, io::BufReader};
use std::sync::OnceLock;
use serde_json::Value;

static INSTANCE: OnceLock<Result<Filter, String>> = OnceLock::new();

const CONFIGURATION : &str = "./config.json";

pub struct Filter {
    ignore: Gitignore,
    base_dir: String,
    current_id: u64,
}

impl Filter {
    fn new() -> Result<Self, String> {
        let conf_file = fs::File::open(CONFIGURATION)
            .map_err(|e| format!("Error: {CONFIGURATION} missing or unreadable.\n{}", e))?;

        let reader = BufReader::new(conf_file);

        let conf: Value = serde_json::from_reader(reader)
            .map_err(|e| format!("Error: {CONFIGURATION} structure is invalid.\n{}", e))?;

        let watch_dir = conf["watch_dir"]
            .as_str()
            .ok_or_else(|| "Error: 'watch_dir' must be a string in config.json.".to_string())?
            .to_string();

        let ignore_rules = conf["ignore"]
            .as_array()
            .ok_or_else(|| "Error: 'ignore' must be an array in config.json.".to_string())?
            .clone();

        let mut ignore_builder = GitignoreBuilder::new(&watch_dir);

        for val in ignore_rules {
            let val_str = val.as_str()
                .ok_or_else(|| "Error: Each value in 'ignore' array must be a string.".to_string())?;

            ignore_builder.add_line(None, val_str)
                .map_err(|e| format!("Error: Invalid ignore rule '{}'. {}", val_str, e))?;
        }

        let ignorer = ignore_builder.build()
            .map_err(|e| format!("Error: Failed to build Gitignore rules. {}", e))?;

        Ok(Filter {
            ignore: ignorer,
            base_dir: watch_dir,
            current_id: 0,
        })
    }

    pub fn get_instance() -> Result<&'static Filter, String> {
        INSTANCE.get_or_init(Filter::new).as_ref().map_err(|e| e.clone())
    }

    pub fn should_filter(&self, path: &str) -> bool {
        self.ignore.matched(path, false).is_ignore()
    }

    pub fn get_watch_dir(&self) -> Result<&str, String> {
        Ok(&self.base_dir)
    }
}
