use libp2p::{
    core::upgrade,
    futures::StreamExt,
    mplex,
    noise::{ Keypair, NoiseConfig, X25519Spec },
    swarm::{ Swarm, SwarmBuilder },
    tcp::TokioTcpConfig,
    Transport,
};
use log::{ error, info };
use std::time::Duration;
use tokio::{ io::{ stdin, AsyncBufReadExt, BufReader }, select, spawn, sync::mpsc, time::sleep };

mod p2p;
mod model {
    pub mod block;
    pub mod blockchain;
    pub mod hash;
}

use crate::{ model::blockchain::Blockchain, p2p::{ BlockchainBehaviour, ChainResponse } };

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    info!("Peer Id: {}", p2p::PEER_ID.clone());

    let (response_sender, mut response_receiver) = mpsc::unbounded_channel();
    let (init_sender, mut init_receiver) = mpsc::unbounded_channel();

    let auth_keys = Keypair::<X25519Spec>
        ::new()
        .into_authentic(&p2p::KEYS)
        .expect("can create auth keys");

    let transport = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(auth_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let behaviour = p2p::BlockchainBehaviour::new(
        Blockchain::new(),
        response_sender,
        init_sender.clone()
    ).await;

    let mut swarm = SwarmBuilder::new(transport, behaviour, *p2p::PEER_ID)
        .executor(
            Box::new(|fut| {
                spawn(fut);
            })
        )
        .build();

    let mut stdin = BufReader::new(stdin()).lines();

    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0".parse().expect("can get a local socket")
    ).expect("swarm can be started");

    spawn(async move {
        sleep(Duration::from_secs(1)).await;
        info!("sending init event");
        init_sender.send(true).expect("can send init event");
    });

    loop {
        let evt = {
            select! {
                line = stdin.next_line() => Some(
                    p2p::EventType::Input(
                        line.expect("can get line").expect("can read line from stdin")
                    )
                ),
                response = response_receiver.recv() => {
                    Some(p2p::EventType::LocalChainResponse(response.expect("response exists")))
                },
                _init = init_receiver.recv() => {
                    Some(p2p::EventType::Init)
                }
                _event = swarm.select_next_some() => {
                    None
                },
            }
        };

        if let Some(event) = evt {
            handle_event(event, &mut swarm);
        }
    }

    fn handle_event(event: p2p::EventType, swarm: &mut Swarm<BlockchainBehaviour>) {
        match event {
            p2p::EventType::Init => handle_init_event(swarm),
            p2p::EventType::LocalChainResponse(resp) => handle_local_chain_response(resp, swarm),
            p2p::EventType::Input(line) => handle_input_event(line, swarm),
        }
    }

    fn handle_init_event(swarm: &mut Swarm<BlockchainBehaviour>) {
        let peers = p2p::get_list_of_peers(swarm);
        swarm.behaviour_mut().blockchain = swarm.behaviour_mut().blockchain.genesis();

        info!("Connected nodes: {}", peers.len());

        if let Some(last_peer) = peers.last() {
            let req = p2p::LocalChainRequest {
                from_peer_id: last_peer.to_string(),
            };

            let json = serde_json::to_string(&req).expect("can jsonify request");

            swarm.behaviour_mut().floodsub.publish(p2p::CHAIN_TOPIC.clone(), json.as_bytes());
        }
    }

    fn handle_local_chain_response(resp: ChainResponse, swarm: &mut Swarm<BlockchainBehaviour>) {
        let json = serde_json::to_string(&resp).expect("can jsonify response");

        swarm.behaviour_mut().floodsub.publish(p2p::CHAIN_TOPIC.clone(), json.as_bytes());
    }

    fn handle_input_event(line: String, swarm: &mut Swarm<BlockchainBehaviour>) {
        match line.as_str() {
            "ls p" => p2p::handle_print_peers(swarm),
            "ls c" => p2p::handle_print_chain(swarm),
            cmd if cmd.starts_with("create block") => p2p::handle_create_block(cmd, swarm),
            _ => error!("unknown command"),
        }
    }
}
