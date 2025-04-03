
#[allow(unused_imports)]
use log::*;
//use crate::actor::tcp_listener::TcpMessage;
#[allow(unused_imports)]
//use std::time::{Duration, SystemTime};
use steady_state::*;
use crate::Args;
use std::error::Error;
use tokio::time::{sleep, Duration};
use crate::actor::tcp_worker::ConfigMsg;
use crate::actor::file_filter::Filter; // Import the Filter struct for connection details
//use std::io;

const BUFFER_SIZE: usize = 4096;

//if no internal state is required (recommended) feel free to remove this.
#[derive(Default)]
pub(crate) struct TcpworkeractorInternalState {
}


pub async fn run(context: SteadyContext
        ,config_conn_tx: SteadyTx<ConfigMsg>
        , state: SteadyState<TcpworkeractorInternalState>
    ) -> Result<(),Box<dyn Error>> {

  // if needed CLI Args can be pulled into state from _cli_args
  let _cli_args = context.args::<Args>();
  // monitor consumes context and ensures all the traffic on the chosen channels is monitored
  // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
  let cmd =  into_monitor!(context, [],[config_conn_tx]);
  internal_behavior(cmd,config_conn_tx,state).await
}


async fn internal_behavior<C: SteadyCommander>(
    mut cmd: C,
    config_conn_tx: SteadyTx<ConfigMsg>,
    _state: SteadyState<TcpworkeractorInternalState>,
) -> Result<(), Box<dyn Error>> {

    let mut _buf = [0;BUFFER_SIZE];

    //let mut config_conn_rx = config_conn_rx.lock().await;
    let mut config_conn_tx = config_conn_tx.lock().await;


    while cmd.is_running(&mut || config_conn_tx.mark_closed()) {
        let _clean = await_for_all!(cmd.wait_vacant(&mut config_conn_tx, BUFFER_SIZE));

        // Retrieve the Filter instance to access configuration details
        let filter = Filter::get_instance();
        
        // Get configuration details from the Filter instance
        let watch_dir: &str = filter.get_watch_dir();
        

        // send data through the channel to tcp_worker
        let _ = cmd.send_async(&mut config_conn_tx, ConfigMsg { text: format!("{}", watch_dir)},SendSaturation::IgnoreAndWait,).await;

        sleep(Duration::from_secs(400)).await;

        cmd.relay_stats();

    }
    Ok(())
}




/* #[cfg(test)]
pub(crate) mod tests {
    use std::time::Duration;
    use steady_state::*;
    use super::*;

    #[async_std::test]
    pub(crate) async fn test_simple_process() {
       let mut graph = GraphBuilder::for_testing().build(());
       let (test_config_conn_tx,config_conn_rx) = graph.channel_builder().with_capacity(1024).build();
       
       let (tcp_msg_tx,test_tcp_msg_rx) = graph.channel_builder().with_capacity(1024).build();
       let state = new_state();
       graph.actor_builder()
                    .with_name("UnitTest")
                    .build_spawn( move |context|
                            internal_behavior(context, config_conn_rx.clone(), tcp_msg_tx.clone(), state.clone())
                     );

       graph.start(); //startup the graph
       //TODO:  adjust this vec content to make a valid test
       test_config_conn_tx.testing_send_all(vec![TcpMessage::default()],true).await;

        
       graph.request_stop();
       graph.block_until_stopped(Duration::from_secs(15));
       //TODO:  confirm values on the output channels
       //    assert_eq!(test_tcp_msg_rx.testing_avail_units().await, 1); // check expected count
       let results_tcp_msg_vec = test_tcp_msg_rx.testing_take().await;
        }
} */