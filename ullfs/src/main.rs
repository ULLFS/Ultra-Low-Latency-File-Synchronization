mod args;
use anyhow::Error;
use aya::{include_bytes_aligned, maps::{Array, AsyncPerfEventArray, PerCpuArray}, programs::KProbe, util::online_cpus, Ebpf};
use bytes::{Buf, BytesMut};
use serde_json::Value;
use structopt::StructOpt;
#[allow(unused_imports)]
use log::*;
use crate::args::Args;
use std::{fs, io::BufReader, process, sync::Arc, time::Duration};
use steady_state::*;
mod client;
pub mod filehasher;
pub mod fileFilter;
pub mod createPacket;
pub mod fileDifs;
pub mod hashFileDif;
pub mod client_tcp;
mod actor {
    
        pub mod ebpf_listener;
        // pub mod tcp_worker;
        // pub mod handle_client;
}

fn main() {
    let opt = Args::from_args();

    let service_executable_name = "ullfs";
    let service_user = "ullfs_user";
    let systemd_command = SystemdBuilder::process_systemd_commands(  opt.systemd_action()
                                                   , opt.to_cli_string(service_executable_name).as_str()
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
        .with_type()
        .with_line_expansion(1.0f32);

    //this common root of the actor builder allows for common config of all actors
    let base_actor_builder = graph.actor_builder()
        .with_mcpu_percentile(Percentile::p80())
        .with_load_percentile(Percentile::p80());

    //build channels
    //{Index, Value, Flags}
    // let t = tokio::signal::ctrl_c().await;
    // println!("Exiting");
    
    //build actors
    
    {
        
        let state = new_state();
        let ebpf_tx: Vec<SteadyTx<String>> = Vec::new();
        let ebpf_rx: Vec<SteadyRx<String>> = Vec::new();
        for cpu_id in online_cpus().map_err(|(_, error)| error).expect("Failed to get online cpus"){
            let (ebpf_listener_conn_tx, ebpf_listener_conn_rx) = base_channel_builder.build();
            ebpf_tx.push(Box::leak(Box::new(ebpf_listener_conn_tx)));
            ebpf_rx.push(ebpf_listener_conn_rx);
        }  
     base_actor_builder.with_name("EbpfListenerActor")
                 .build( move |context| actor::ebpf_listener::run(context
                                            , ebpf_tx
                                            , state.clone())
                  , &mut Threading::Spawn );
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
