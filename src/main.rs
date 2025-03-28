mod args;
use structopt::StructOpt;
#[allow(unused_imports)]
use log::*;
use crate::args::Args;
use std::time::Duration;
use steady_state::*;

mod actor {
    
        pub mod tcp_listener;
        pub mod tcp_worker;
        pub mod handle_client;
}

fn main() {
    let opt = Args::from_args();

    let service_executable_name = "simple_tcp_server";
    let service_user = "simple_tcp_server_user";
    let systemd_command = SystemdBuilder::process_systemd_commands(  opt.systemd_action()
                                                        ,service_executable_name
                                                        ,service_user);

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
    .with_filled_trigger(Trigger::AvgBelow(Filled::p90()), AlertColor::Red)
    .with_type()
    .with_line_expansion(1.0f32);

    //this common root of the actor builder allows for common config of all actors
    let base_actor_builder = graph.actor_builder()
        /* .with_mcpu_percentile(Percentile::p80())
        .with_load_percentile(Percentile::p80()); */
        .with_thread_info();


    //build channels
    
    let (tcplisteneractor_tcp_conn_tx, tcpworkeractor_tcp_conn_rx) = base_channel_builder
        .with_capacity(1024)
        .build();

    //build actors
    
    {
     let state = new_state();
    
     base_actor_builder.with_name("Tcp Listener")
                 .build( move |context| actor::tcp_listener::run(context
                                            , tcplisteneractor_tcp_conn_tx.clone()
                                            , state.clone() )
                  , &mut Threading::Spawn );
    }

    {
     let state = new_state();
    
     base_actor_builder.with_name("Tcp Worker")
                 .build( move |context| actor::tcp_worker::run(context
                                            , tcpworkeractor_tcp_conn_rx.clone()
                                            , state.clone() )
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