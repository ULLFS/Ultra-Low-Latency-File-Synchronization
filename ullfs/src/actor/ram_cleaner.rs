use std::{error::Error, time::Duration};

// use tokio::task::sleep;
use steady_state::*;
use tokio::time::sleep;
use crate::{fileDifs,Args};
use super::ebpf_listener::RuntimeState;

pub fn check_ram(num_minutes_passed: u32) -> bool{
    println!("Checking ram");
    let files = fileDifs::FileData::get_instance();
    files.clean_ram(num_minutes_passed)
}

pub async fn run(context: SteadyContext
    ,state: SteadyState<RuntimeState>) -> Result<(), Box<dyn Error>> {
        // if needed CLI Args can be pulled into state from _cli_args
        let _cli_args = context.args::<Args>();
        // monitor consumes context and ensures all the traffic on the chosen channels is monitored
        // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
        let cmd =  into_monitor!(context, [],[]);
        internal_behavior(cmd,state).await
}

async fn internal_behavior <C: SteadyCommander>( 
    mut cmd: C,
    _state: SteadyState<RuntimeState>
) -> Result<(),Box<dyn Error>> {

    // println!("Running ram cleaner!");
    let mut num_minutes_passed = 1;
    while cmd.is_running(move || {
        return true; // I assume there is something else going on in is_running so using this instead of a loop
    }) {
        sleep(Duration::from_secs(60)).await; // Hopefully this has no issues using tokio here
        
        // Doesnt seem to
        // Sleep for 60 seconds
        if check_ram(num_minutes_passed){
            num_minutes_passed = 1;
        } else {
            println!("Skipping a ram cleanup");
            num_minutes_passed += 1;
        }
    }

    Ok(())
}