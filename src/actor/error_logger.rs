
#[allow(unused_imports)]
use log::*;
//use crate::actor::tcp_listener::TcpMessage;
#[allow(unused_imports)]
//use std::time::{Duration, SystemTime};
use steady_state::*;
use tokio::sync::broadcast::error;
use crate::Args;
use std::error::Error;
use tokio::time::{sleep, Duration};
use crate::actor::tcp_worker::ConfigMsg;
use crate::actor::file_filter::Filter;

use super::tcp_listener; // Import the Filter struct for connection details
//use std::io;

const BUFFER_SIZE: usize = 4096;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub(crate) struct ErrorMessage {
   pub(crate) text: String
}


pub async fn run(context: SteadyContext
        , tcp_listener_rx: SteadyRx<ErrorMessage>
        , tcp_worker_rx: SteadyRx<ErrorMessage>
        , config_checker_rx: SteadyRx<ErrorMessage>
    ) -> Result<(),Box<dyn Error>> {

  // if needed CLI Args can be pulled into state from _cli_args
  let _cli_args = context.args::<Args>();
  // monitor consumes context and ensures all the traffic on the chosen channels is monitored
  // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
  let cmd =  into_monitor!(context, [tcp_listener_rx, tcp_worker_rx, config_checker_rx],[]);
  internal_behavior(cmd, tcp_listener_rx, tcp_worker_rx, config_checker_rx).await
}


async fn internal_behavior<C: SteadyCommander>(
    mut cmd: C,
    tcp_listener_rx: SteadyRx<ErrorMessage>,
    tcp_worker_rx: SteadyRx<ErrorMessage>,
    config_checker_rx: SteadyRx<ErrorMessage>,
) -> Result<(), Box<dyn Error>> {

    let mut _buf = [0;BUFFER_SIZE];

    //let mut config_conn_rx = config_conn_rx.lock().await;
    let mut tcp_listener_rx = tcp_listener_rx.lock().await;
    let mut tcp_worker_rx = tcp_worker_rx.lock().await;
    let mut config_checker_rx = config_checker_rx.lock().await;


    while cmd.is_running(&mut || tcp_listener_rx.is_closed_and_empty() && tcp_worker_rx.is_closed_and_empty() && config_checker_rx.is_closed_and_empty()) {
        let clean = await_for_all!(cmd.wait_avail(&mut tcp_listener_rx,5)
                                        ,cmd.wait_avail(&mut tcp_worker_rx, 5)
                                        ,cmd.wait_avail(&mut config_checker_rx, 5) );

        match cmd.try_take(&mut tcp_listener_rx) {
            Some(message) => {
                error!("Error: {:?}", message);
                cmd.relay_stats();
            },
            None => {
                if clean {
                    error!("internal error, should have found message");
                }
            }
        }

        match cmd.try_take(&mut tcp_worker_rx) {
            Some(message) => {
                error!("Error: {:?}", message);
                cmd.relay_stats();
            },
            None => {
                if clean {
                    error!("internal error, should have found message");
                }
            }
        }

        match cmd.try_take(&mut config_checker_rx) {
            Some(message) => {
                error!("Error: {:?}", message);
                cmd.relay_stats();
            },
            None => {
                if clean {
                    error!("internal error, should have found message");
                }
            }
        }

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