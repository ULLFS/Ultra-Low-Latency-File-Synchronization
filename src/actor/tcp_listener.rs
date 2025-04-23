
use std::cmp::{PartialEq, PartialOrd};
//use async_std::stream;
#[allow(unused_imports)]
use log::*;
/* use tokio::runtime::Runtime;
use std::default; */
#[allow(unused_imports)]
use std::time::Duration;
use steady_state::*;
use crate::Args;
use std::{error::Error, fmt::format};
use tokio::{net::{TcpListener, TcpStream}, sync::broadcast::error};

const BATCH_SIZE: usize = 7000;

#[derive(Copy, Clone)]
pub(crate) struct RuntimeState {
    value: u64,
    buffer: [u8; BATCH_SIZE], // Use a byte buffer for TCP streams
}

impl RuntimeState {
    pub(crate) fn new(value: i32) -> Self {
        RuntimeState {
            value: value as u64,
            buffer: [0; BATCH_SIZE], // Initialize the byte buffer
        }
    }
}

pub async fn run(context: SteadyContext
        ,tcp_conn_tx: SteadyTx<TcpStream>
        , state: SteadyState<RuntimeState>
    ) -> Result<(),Box<dyn Error>> {

  // if needed CLI Args can be pulled into state from _cli_args
  let _cli_args = context.args::<Args>();
  // monitor consumes context and ensures all the traffic on the chosen channels is monitored
  // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
  let listener = TcpListener::bind("127.0.0.1:34254").await?;
  let cmd =  into_monitor!(context, [],[tcp_conn_tx]);
  internal_behavior(cmd,tcp_conn_tx,listener, state).await
}

async fn internal_behavior<C: SteadyCommander>(
    mut cmd: C,
    tcp_conn_tx: SteadyTx<TcpStream>,
    listener: TcpListener,
    state: SteadyState<RuntimeState>,
) -> Result<(), Box<dyn Error>> {
    //let mut state_guard = steady_state(&state, || RuntimeState::new(1)).await;

    //let mut state = state.lock(|| RuntimeState::new(1)).await;
    let mut _state = state.lock().await;

    //if let Some(mut _state) = state_guard.as_mut() {
    let mut tcp_conn_tx = tcp_conn_tx.lock().await;

    println!("(tcp_listener) Listening on port 34254...");

    let tcp_vacant_block = BATCH_SIZE.min(tcp_conn_tx.capacity());

    while cmd.is_running(&mut || tcp_conn_tx.mark_closed()) {

        let _clean = await_for_all!(cmd.wait_vacant(&mut tcp_conn_tx, tcp_vacant_block));

        let (stream) = match listener.accept().await{
            Ok(x) => {
                println!("(tcp_listener) Attemping to forward the connection over to the tcp_worker");
                let _done = cmd.send_async(&mut tcp_conn_tx, x.0, SendSaturation::IgnoreAndWait).await;
                cmd.relay_stats();
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    continue;   
                } else {
                    panic!("(tcp_listener) Error accepting connection: {:?}", e);
                }
            }
        };
    }
            
    Ok(())
}
