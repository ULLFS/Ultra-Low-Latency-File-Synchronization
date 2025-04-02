use std::error::Error;

use steady_state::{SteadyContext, SteadyState};

use super::ebpf_listener::RuntimeState;

pub async fn run(context: SteadyContext, 
    // transmitter: SteadyTx<Box<String>>,
    state: SteadyState<RuntimeState>) -> Result<(),Box<dyn Error>> {
    

    Ok(())
}