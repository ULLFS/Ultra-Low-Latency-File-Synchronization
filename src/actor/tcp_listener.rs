
#[allow(unused_imports)]
use log::*;
/* use tokio::runtime::Runtime;
use std::default; */
#[allow(unused_imports)]
use std::time::Duration;
use steady_state::*;
use crate::Args;
use std::error::Error;
//use crate::actor::tcp_worker::TcpResponse;
use tokio::net::{TcpListener, TcpStream};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
//use std::io::{Read,Write};
//use std::sync::Arc;

const BATCH_SIZE: usize = 7000;

#[derive(Default,Clone,Debug,Eq,PartialEq)]
pub(crate) struct TcpMessage {
    data: u64
}

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
        ,tcp_msg_rx: SteadyRx<TcpStream>
        ,tcp_conn_tx: SteadyTx<TcpStream>, state: SteadyState<RuntimeState>
    ) -> Result<(),Box<dyn Error>> {

  // if needed CLI Args can be pulled into state from _cli_args
  let _cli_args = context.args::<Args>();
  // monitor consumes context and ensures all the traffic on the chosen channels is monitored
  // monitor and context both implement SteadyCommander. SteadyContext is used to avoid monitoring
  let listener = TcpListener::bind("127.0.0.1:34254").await?;
  let cmd =  into_monitor!(context, [tcp_msg_rx],[tcp_conn_tx]);
  internal_behavior(cmd, tcp_msg_rx, tcp_conn_tx, listener, state).await
}

async fn internal_behavior<C: SteadyCommander>(
    mut cmd: C,
    tcp_msg_rx: SteadyRx<TcpStream>,
    tcp_conn_tx: SteadyTx<TcpStream>,
    listener: TcpListener,
    state: SteadyState<RuntimeState>,
) -> Result<(), Box<dyn Error>> {
    let mut state_guard = steady_state(&state, || RuntimeState::new(1)).await;

    if let Some(mut _state) = state_guard.as_mut() {
        let mut tcp_msg_rx = tcp_msg_rx.lock().await;
        let mut tcp_conn_tx = tcp_conn_tx.lock().await;

        println!("(tcp_listener) Listening on port 34254...");

        while cmd.is_running(&mut || 
            tcp_msg_rx.is_closed_and_empty() && tcp_conn_tx.mark_closed()
        ) {
            let (stream, _) = match listener.accept().await{
                Ok(x) => x,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        continue;
                    } else {
                        panic!("(tcp_listener) Error accepting connection: {:?}", e);
                    }
                }
            };
            //let _clean = await_for_all!(cmd.wait_vacant_units(&mut tcp_conn_tx, 1024));
            println!("(tcp_listener) Attemping to forward the connection over to the tcp_worker");
            let _done = cmd.send_async(&mut tcp_conn_tx, stream, SendSaturation::IgnoreAndWait).await;
        }
            
    } else {
        warn!("missing state, unable to start actor");
    }
    Ok(())
}




#[cfg(test)]
pub(crate) mod tests {
    use std::time::Duration;
    /* use futures::sink::Buffer; */
    use steady_state::*;
    /* use crate::actor::tcp_listener; */

    use super::*;

    #[async_std::test]
    pub(crate) async fn test_simple_process() {
        let mut graph = GraphBuilder::for_testing().with_telemetry_metric_features(false).build(());
        let listener = TcpListener::bind("127.0.0.1:7878");
        // Set up the channels
        let (tcp_conn_tx, tcp_msg_rx) = graph.channel_builder().with_capacity(1000).build();
        
        // Create state
        let state = new_state();
        
        // Build the actor to spawn the internal behavior
        graph.actor_builder()
            .with_name("TcpListener")
            .build_spawn(move |context| 
                internal_behavior(context, tcp_conn_tx.clone(), tcp_msg_rx.clone(), listener, state)
            );

        // Start the graph
        graph.start();

        // Simulate a TCP connection
        let stream = TcpStream::connect("127.0.0.1:7878").await.unwrap();
        tcp_conn_tx.send(stream).await.unwrap();

        graph.request_stop(); // Request the graph to stop
        graph.block_until_stopped(Duration::from_secs(15)); // Wait for the graph to stop
        
        /* // Prepare a test message (not the default TcpResponse)
        let test_message = TcpResponse {
            _dummy: 1, // You can add actual data here
        };

        // Send the test message to the tcp_msg_tx channel
        tcp_msg_tx.testing_send_all(vec![test_message], true).await; */

        // Allow some time for processing
        /* graph.block_until_stopped(Duration::from_secs(15)); */

        // Verify that the tcp_conn_tx received the forwarded message
        /* let results_tcp_conn_vec = test_tcp_conn_rx.testing_take().await; */
        
        // Add assertions to verify that the message was forwarded properly
        /* assert_eq!(results_tcp_conn_vec.len(), 1); // We expect 1 message to be forwarded
        assert_eq!(results_tcp_conn_vec[0]._dummy, 1); // Check if the message content is as expected */
    }
}


/* match stream.read(&mut buffer) {
                Ok(0) => {
                    println!("(tcp_listener) Client disconnected: {}", addr);
                }
                Ok(size) => {
                    let message = TcpMessage {
                        data: buffer[..size].to_vec(), // Store the actual received data
                    };

                    match cmd.try_send(&mut tcp_conn_tx, message.clone()) {
                        Ok(()) => {
                            // Log the forwarded message again after sending
                            //println!("Successfully forwarded message from {} to worker", addr);
                            let message_str = String::from_utf8_lossy(&message.data); // This will safely convert bytes to a string
                            let message_str = message_str.trim_end();
                            println!("(tcp_listener) Forwarded message: {} from {} to worker", message_str, addr);
                            cmd.request_graph_stop();
                            
                        }
                        Err(err) => {
                            println!("(tcp_listener) Error sending message: {:?}", err);
                        }
                    }
                }
                Err(e) => {
                    println!("(tcp_listener) Error reading from {}: {}", addr, e);
                }
            }    */  


// Accept new connections asynchronously
            /* match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("(tcp_listener) New client: {addr}");

                    let _stream = Arc::new(stream); // Wrap in Arc if needed

                    let msg = format!("New client: {addr}");

                    // Send the message to the worker channel
                    /* match cmd.try_send(&mut tcp_conn_tx, msg.clone()) {
                        Ok(()) => {
                            println!("(tcp_listener) Successfully forwarded connection from {addr} to worker");
                        }
                        Err(e) => {
                            println!("(tcp_listener) Error sending connection: {:?}", e);
                        }
                    } */

                    // Send the stream to the worker channel
                    /* match cmd.try_send(&mut tcp_conn_tx, stream.clone()) {
                        Ok(()) => {
                            println!("(tcp_listener) Successfully forwarded connection from {addr} to worker");
                        }
                        Err(e) => {
                            println!("(tcp_listener) Error sending connection: {:?}", e);
                        }
                    }
                   */
                }
                Err(e) => println!("(tcp_listener) couldn't get client: {:?}", e),
            } */
