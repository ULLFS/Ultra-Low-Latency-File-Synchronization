use std::error::Error;

use steady_state::{SteadyContext, SteadyRx, SteadyState};

use super::ebpf_listener::RuntimeState;
pub async fn run(context: SteadyContext, 
    ebpf_receiver: SteadyRx<Box<String>>,
    // tcp_receiver: SteadyRx<Box<String>>,
    state: SteadyState<RuntimeState>) -> Result<(),Box<dyn Error>> {
    //Expects received data to be in the following format:
    // Send_type\0Filepath
    // Send Type can be FULL_FILE, CREATE_DIRECTORY, RENAME_DIR, RENAME_FILE,
    // DELTA_FILE, DELETE_FILE, DELETE_DIR, AUTO_FILE
    // More to come possibly
    // AUTO_FILE specifically will automatically determine whether to send a delta file or full file
    

    Ok(())
}