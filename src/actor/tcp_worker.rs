
#[allow(unused_imports)]
use log::*;
//use crate::actor::tcp_listener::TcpMessage;
#[allow(unused_imports)]
use std::time::Duration;
use steady_state::*;
use crate::{actor::handle_client, Args};
use std::error::Error;
use tokio::net::TcpStream;
//use std::io;

const BUFFER_SIZE: usize = 4096;

#[derive(Default,Clone,Debug,Eq,PartialEq)]
pub(crate) struct TcpResponse {
   pub data : Vec<u8>
}

//if no internal state is required (recommended) feel free to remove this.
#[derive(Default)]
pub(crate) struct TcpworkeractorInternalState {
}

#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub(crate) struct  ConfigMsg{
        pub(crate) text: String
}


pub async fn run(context: SteadyContext
        ,tcp_conn_rx: SteadyRx<TcpStream>
        ,tcp_conn_config_rx: SteadyRx<ConfigMsg>
        , state: SteadyState<TcpworkeractorInternalState>
    ) -> Result<(),Box<dyn Error>> {

  // if needed CLI Args can be pulled into state from _cli_args
  let _cli_args = context.args::<Args>();
  // monitor consumes context and ensures all the traffic on the chosen channels is monitored
  // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
  let cmd =  into_monitor!(context, [&tcp_conn_rx, &tcp_conn_config_rx],[]);
  internal_behavior(cmd,tcp_conn_rx, tcp_conn_config_rx, state).await
}

async fn internal_behavior<C: SteadyCommander>(
    mut cmd: C,
    tcp_conn_rx: SteadyRx<TcpStream>,
    _tcp_conn_config_rx: SteadyRx<ConfigMsg>,
    _state: SteadyState<TcpworkeractorInternalState>,
) -> Result<(), Box<dyn Error>> {

    let mut buf = [0;BUFFER_SIZE];

    let mut tcp_conn_rx = tcp_conn_rx.lock().await;
    let mut _tcp_conn_config_rx = _tcp_conn_config_rx.lock().await;

    while cmd.is_running(&mut || tcp_conn_rx.is_closed_and_empty() && _tcp_conn_config_rx.is_closed_and_empty()) {
        //let clean = await_for_all!(cmd.wait_avail(&mut tcp_conn_rx, 1)    );

        // I need to ask Nathan a question about what the count should be. 
        let clean = await_for_any!(cmd.wait_avail(&mut tcp_conn_rx, 1),
                                               cmd.wait_avail(&mut _tcp_conn_config_rx,1)); // count: The number of units to wait for

        match cmd.try_take(&mut _tcp_conn_config_rx){
            Some(msg) => {
                println!("(tcp_worker) {:?}", msg);
                cmd.relay_stats();
            }
            None => {
                continue;
            }
        }
        
        match cmd.try_take(&mut tcp_conn_rx) {
            Some(stream) => {
                println!("(tcp_worker) Successfully forwarded connection from tcp_listener to tcp_worker.");
                println!("(tcp_worker) New client's address: {:?}", stream.peer_addr()?);
                //let mut std_tcp_stream = stream.into_std()?;
                /* std_tcp_stream.set_nonblocking(false)?;
                std_tcp_stream.read_exact(&mut buf)? */
                loop {
                    let _ = handle_client::processing(&stream);
                    cmd.relay_stats();
                    /* stream.readable().await?;
                    match stream.try_read(&mut buf) {
                        Ok(0) => {
                            println!("(tcp_worker) Connection closed by {:?}", stream.peer_addr()?);
                            break;
                        },
                        Ok(_n) => {
                            let _ = handle_client::processing(&std_tcp_stream);
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                            return Err(e.into());
                        }
                    } */
                }
            }
            None => {
                if clean {
                    error!("internal error, should have found message");
                }
            }
        };
    }
    Ok(())
}




#[cfg(test)]
pub(crate) mod tests {
    use std::time::Duration;
    use steady_state::*;
    use super::*;

    #[async_std::test]
    pub(crate) async fn test_simple_process() {
       let mut graph = GraphBuilder::for_testing().build(());
       let (test_tcp_conn_tx,tcp_conn_rx) = graph.channel_builder().with_capacity(1024).build();
       
       let (tcp_msg_tx,test_tcp_msg_rx) = graph.channel_builder().with_capacity(1024).build();
       let state = new_state();
       graph.actor_builder()
                    .with_name("UnitTest")
                    .build_spawn( move |context|
                            internal_behavior(context, tcp_conn_rx.clone(), tcp_msg_tx.clone(), state.clone())
                     );

       graph.start(); //startup the graph
       //TODO:  adjust this vec content to make a valid test
       test_tcp_conn_tx.testing_send_all(vec![TcpMessage::default()],true).await;

        
       graph.request_stop();
       graph.block_until_stopped(Duration::from_secs(15));
       //TODO:  confirm values on the output channels
       //    assert_eq!(test_tcp_msg_rx.testing_avail_units().await, 1); // check expected count
       let results_tcp_msg_vec = test_tcp_msg_rx.testing_take().await;
        }
}