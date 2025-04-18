mod args;
use anyhow::Error;
use aya::{include_bytes_aligned, maps::{Array, AsyncPerfEventArray, PerCpuArray}, programs::KProbe, util::online_cpus, Ebpf};
use bytes::{Buf, BytesMut};
use serde_json::Value;
use structopt::StructOpt;
#[allow(unused_imports)]
use log::*;
use tokio::net::TcpStream;
use crate::args::Args;
use std::{fs, io::BufReader, process, sync::Arc, thread::Thread, time::Duration};
use steady_state::*;
mod client;
pub mod filehasher;
pub mod fileFilter;
pub mod createPacket;
pub mod fileDifs;
pub mod hashFileDif;
pub mod client_tcp;
pub mod ebpf_setup;

mod actor {
    
        pub mod ebpf_listener;
        pub mod ram_cleaner;
        pub mod tcp_sender;
        pub mod connection_handler;
}
// #[derive(PartialEq, Eq, Clone)]
pub struct TcpChannel {
    stream: TcpStream,
    name: String
}
fn main() {
    let opt = Args::from_args();

    let service_executable_name = "ullfs";
    let service_user = "ullfs_user";
    let systemd_command = SystemdBuilder::process_systemd_commands(  opt.systemd_action()
                                                   , service_executable_name
                                                   , service_user);

    if !systemd_command {

        info!("Starting up");
        
        let mut graph = build_graph(GraphBuilder::for_production()
                                .with_telemtry_production_rate_ms(200)
                                .build(opt.clone()) );

        graph.start();


        graph.block_until_stopped(Duration::from_secs(2));
    }
}

fn build_graph(mut graph: Graph) -> Graph {

    //this common root of the channel builder allows for common config of all channels
    let base_channel_builder = graph.channel_builder()
        .with_filled_trigger(Trigger::AvgAbove(Filled::p90()), AlertColor::Red)
        .with_filled_trigger(Trigger::AvgAbove(Filled::percentage(75.00f32).expect("internal range error")), AlertColor::Orange)
        .with_filled_trigger(Trigger::AvgAbove(Filled::p50()), AlertColor::Yellow)
        .with_line_expansion(0.0001f32)
        .with_type();

    //this common root of the actor builder allows for common config of all actors
    let base_actor_builder = graph.actor_builder()
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m256()), AlertColor::Yellow)
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m768()), AlertColor::Orange)
        .with_mcpu_trigger(Trigger::AvgAbove( MCPU::m1024()), AlertColor::Red)
        .with_thread_info()
        .with_mcpu_avg()
        .with_load_avg();


    //build channels
    let (ebpf_listener_conn_tx, ebpf_listener_conn_rx) = base_channel_builder
            .with_capacity(1024)
            .build();

    let (tcp_listener_conn_tx, tcp_listener_conn_rx) = base_channel_builder
            .with_capacity(1024)
            .build();

    let (connection_handler_conn_tx, connection_handler_conn_rx) = base_channel_builder
            .with_capacity(1024)
            .build();

   
    //build actors
    {
        // Detects all changes in the file system thanks to eBPF.
        let state = new_state();

        base_actor_builder.with_name("Ebpf Listener")
                 .build( move |context| actor::ebpf_listener::run(context
                                            , ebpf_listener_conn_tx.clone()
                                            , state.clone())
                  , &mut Threading::Spawn );

    }
    {
        // Cleans the ram on a timeout, every minute looping through each of our files stored in ram to see if we need to clean
        let state = new_state();
        base_actor_builder.with_name("RAM Cleaner")
                    .build(move | context | actor::ram_cleaner::run(context,
                                             state.clone())
                    ,&mut Threading::Spawn);

    }

    {
        // Sends data over TCP. Just tell it what file to send and it should handle it from there
        let state: Arc<futures_util::lock::Mutex<Option<actor::ebpf_listener::RuntimeState>>> = new_state();
        base_actor_builder.with_name("TCP Sender")
                        .build(move | context | actor::tcp_sender::run(context, 
                            ebpf_listener_conn_rx.clone(), 
                            tcp_listener_conn_rx.clone(),
                            connection_handler_conn_tx.clone(),
                            state.clone()
                        )
                    ,&mut Threading::Spawn);

    }

    {
        // Listens over TCP, will handle reconnecting when connections are lost and full file transmission in case of error
        let state = new_state();
        base_actor_builder.with_name("Connection handler")
                            .build(move | context | actor::connection_handler::run(context, 
                                tcp_listener_conn_tx.clone(),
                                connection_handler_conn_rx.clone(),
                                state.clone())
                    ,&mut Threading::Spawn);
        
    }
    
    graph
    
}

#[cfg(test)]
mod graph_tests {
    use async_std::test;
    use steady_state::*;
    use std::time::Duration;
    use crate::args::Args;
    use crate::build_graph;
    use std::ops::DerefMut;
    use futures_timer::Delay;

    #[test]
    async fn test_graph_one() {

            let test_ops = Args {
                loglevel: "debug".to_string(),
                systemd_install: false,
                systemd_uninstall: false,
            };
            let mut graph = build_graph( GraphBuilder::for_testing().build(test_ops.clone()) );
            graph.start();
            let mut guard = graph.sidechannel_director().await;
            let g = guard.deref_mut();
            assert!(g.is_some(), "Internal error, this is a test so this back channel should have been created already");
            if let Some(_plane) = g {

             //NOTE: to ensure the node_call is for the correct channel for a given actor unique types for each channel are required

            

              // //TODO:   if needed you may want to add a delay right here to allow the graph to process the message
              Delay::new(Duration::from_millis(100)).await;

             

            }
            drop(guard);
            graph.request_stop();
            graph.block_until_stopped(Duration::from_secs(3));
        }
    }
