use std::{error::Error, time::Duration};

// use tokio::task::sleep;
use steady_state::*;
use tokio::time::sleep;

use crate::fileDifs;

use super::ebpf_listener::RuntimeState;
pub fn check_ram(num_minutes_passed: u32) -> bool{
    println!("Checking ram");
    let files = fileDifs::FileData::get_instance();
    files.clean_ram(num_minutes_passed)
}
pub async fn run(context: SteadyContext, 
    // transmitter: SteadyTx<Box<String>>,
    state: SteadyState<RuntimeState>) -> Result<(),Box<dyn Error>> {
    let mut cmd =  into_monitor!(context, [],[]);
    println!("Running ram cleaner!");
    let mut num_minutes_passed = 1;
    while cmd.is_running(move || {
        return true;
    }) {
        sleep(Duration::from_secs(60)).await; // Hopefully this has no issues using tokio here
        // Doesnt seem to
        // Sleep for 60 seconds
        if(check_ram(num_minutes_passed)){
            num_minutes_passed = 1;
        } else {
            println!("Skipping a ram cleanup");
            num_minutes_passed += 1;
        }
    }



    Ok(())
}