
#[allow(unused_imports)]
use log::*;
//use crate::actor::tcp_listener::TcpMessage;
#[allow(unused_imports)]
use std::time::Duration;
use steady_state::*;
use crate::{actor::handle_client, Args};
use std::error::Error;
use tokio::net::TcpStream;
use crate::actor::tcp_listener::RuntimeState;

use super::file_filter::Filter;

const BUFFER_SIZE: usize = 4096;

pub async fn run(context: SteadyContext
        ,tcp_conn_rx: SteadyRx<TcpStream>
        ,state: SteadyState<RuntimeState>
    ) -> Result<(),Box<dyn Error>> {

  // if needed CLI Args can be pulled into state from _cli_args
  let _cli_args = context.args::<Args>();
  // monitor consumes context and ensures all the traffic on the chosen channels is monitored
  // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
  let cmd =  into_monitor!(context, [tcp_conn_rx],[]);
  internal_behavior(cmd,tcp_conn_rx,state).await
}

async fn internal_behavior<C: SteadyCommander>(
    mut cmd: C,
    tcp_conn_rx: SteadyRx<TcpStream>,
    state: SteadyState<RuntimeState>,
) -> Result<(), Box<dyn Error>> {

    let mut buf = [0;BUFFER_SIZE];

    let mut tcp_conn_rx = tcp_conn_rx.lock().await;

    let filter = Filter::get_instance();
    let save_path = filter.get_base_dir();

    while cmd.is_running(&mut || tcp_conn_rx.is_closed_and_empty()) {
 
        let clean = await_for_any!(cmd.wait_avail(&mut tcp_conn_rx, 1));

        
        match cmd.try_take(&mut tcp_conn_rx) {
            Some(mut stream) => {
                println!("(tcp_worker) Successfully forwarded connection from tcp_listener to tcp_worker.");
                println!("(tcp_worker) New client's address: {:?}", stream.peer_addr()?);
                let _ = handle_client::processing(stream, &save_path, &mut cmd).await;
                cmd.relay_stats();
            },
            None => {
                if clean {
                    error!("internal error, should have found message");
                }
            }
        };
    }
    Ok(())
}