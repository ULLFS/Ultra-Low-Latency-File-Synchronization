#[allow(unused_imports)]
use log::*;
#[allow(unused_imports)]
use steady_state::*;
use tokio::runtime::Runtime;
use crate::Args;
use std::error::Error;
use crate::actor::tcp_worker::ConfigMsg;
use crate::actor::file_filter::Filter;
use crate::actor::error_logger::ErrorMessage;
use std::time::Duration;
use tokio::time::sleep;

use super::tcp_listener::RuntimeState;

const BUFFER_SIZE: usize = 4096;

pub async fn run(context: SteadyContext
        ,config_conn_tx: SteadyTx<ConfigMsg>
        ,error_conn_tx: SteadyTx<ErrorMessage>
    ) -> Result<(),Box<dyn Error>> {

  // if needed CLI Args can be pulled into state from _cli_args
  let _cli_args = context.args::<Args>();
  // monitor consumes context and ensures all the traffic on the chosen channels is monitored
  // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
  let cmd =  into_monitor!(context, [],[config_conn_tx, error_conn_tx]);
  internal_behavior(cmd,config_conn_tx,error_conn_tx).await
}


async fn internal_behavior <C: SteadyCommander>(
    mut cmd: C,
    config_conn_tx: SteadyTx<ConfigMsg>,
    error_conn_tx: SteadyTx<ErrorMessage>,
) -> Result<(), Box<dyn Error>> {

    let mut _buf = [0;BUFFER_SIZE];

    let mut config_conn_tx = config_conn_tx.lock().await;
    let mut error_conn_tx = error_conn_tx.lock().await;


    while cmd.is_running(&mut || config_conn_tx.mark_closed() &&  error_conn_tx.mark_closed()) {
        
        let _clean = await_for_all!(cmd.wait_vacant(&mut config_conn_tx, BUFFER_SIZE));

        // Get configuration details from the Filter instance
        let watch_dir: &str = match Filter::get_instance(){
            Ok(filter) => match filter.get_watch_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    let _ = cmd.send_async(&mut error_conn_tx
                        , ErrorMessage {text: format!("{}", e)}
                        , SendSaturation::IgnoreAndWait
                    ).await;
                    cmd.relay_stats();
                    continue;
                }
                
            },
            Err(e) => {
                let _ = cmd.send_async(&mut error_conn_tx
                    , ErrorMessage { text: format!("{}", e) }
                    , SendSaturation::IgnoreAndWait
                );
                cmd.relay_stats();
                continue;
            }
        };
        

        // send data through the channel to tcp_worker
        let _ = cmd.send_async(&mut config_conn_tx, ConfigMsg { text: format!("{}", watch_dir)},SendSaturation::IgnoreAndWait,).await;
        cmd.relay_stats();

        println!("(config_checker) this message should output every 5 mins!!!");
        sleep(Duration::from_secs(300)).await;

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