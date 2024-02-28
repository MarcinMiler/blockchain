use libp2p::{
    floodsub::{ Floodsub, FloodsubEvent, FloodsubMessage, Topic },
    identity,
    mdns::{ Mdns, MdnsEvent },
    swarm::{ NetworkBehaviourEventProcess, Swarm },
    NetworkBehaviour,
    PeerId,
};
use log::{ error, info };
use once_cell::sync::Lazy;
use serde::{ Deserialize, Serialize };
use std::collections::HashSet;
use tokio::sync::mpsc;

use crate::model::{ block::Block, blockchain::Blockchain };

pub static KEYS: Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
pub static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(KEYS.public()));
pub static CHAIN_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("CHAINS"));
pub static BLOCK_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("BLOCKS"));

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainResponse {
    pub blocks: Vec<Block>,
    pub receiver: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalChainRequest {
    pub from_peer_id: String,
}

pub enum EventType {
    LocalChainResponse(ChainResponse),
    Input(String),
    Init,
}

#[derive(NetworkBehaviour)]
pub struct BlockchainBehaviour {
    pub floodsub: Floodsub,
    pub mdns: Mdns,

    #[behaviour(ignore)]
    pub response_sender: mpsc::UnboundedSender<ChainResponse>,

    #[behaviour(ignore)]
    pub init_sender: mpsc::UnboundedSender<bool>,

    #[behaviour(ignore)]
    pub blockchain: Blockchain,
}

impl BlockchainBehaviour {
    pub async fn new(
        blockchain: Blockchain,
        response_sender: mpsc::UnboundedSender<ChainResponse>,
        init_sender: mpsc::UnboundedSender<bool>
    ) -> Self {
        let mut behaviour = Self {
            blockchain,
            floodsub: Floodsub::new(*PEER_ID),
            mdns: Mdns::new(Default::default()).await.expect("can create mdns"),
            response_sender,
            init_sender,
        };

        behaviour.floodsub.subscribe(CHAIN_TOPIC.clone());
        behaviour.floodsub.subscribe(BLOCK_TOPIC.clone());

        behaviour
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for BlockchainBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                discovered_list.into_iter().for_each(|(peer_id, _addr)| {
                    self.floodsub.add_node_to_partial_view(peer_id);
                });
            }
            MdnsEvent::Expired(expired_list) => {
                expired_list.into_iter().for_each(|(peer_id, _addr)| {
                    if !self.mdns.has_node(&peer_id) {
                        self.floodsub.remove_node_from_partial_view(&peer_id);
                    }
                });
            }
        }
    }
}

fn handle_chain_response(msg: &FloodsubMessage, resp: ChainResponse, blockchain: &mut Blockchain) {
    info!("Response from: {}", msg.source);

    resp.blocks.iter().for_each(|r| info!("{:?}", r));

    let remote_blockchain = Blockchain { blocks: resp.blocks };

    *blockchain = blockchain.choose_chain(blockchain.clone(), remote_blockchain);
}

fn handle_local_chain_request(
    msg: &FloodsubMessage,
    blockchain: &Blockchain,
    response_sender: &mut mpsc::UnboundedSender<ChainResponse>
) {
    info!("Sending local chain to: {}", msg.source.to_string());

    let send_result = response_sender.send(ChainResponse {
        blocks: blockchain.blocks.clone(),
        receiver: msg.source.to_string(),
    });

    match send_result {
        Ok(_) => info!("Local chain sent"),
        Err(e) => error!("Failed to send local chain: {}", e),
    }
}

fn handle_received_block(block: Block, blockchain: &mut Blockchain) {
    info!("Received block from: {}", block.header.hash);

    match blockchain.add_block(block) {
        Ok(_) => info!("Block added to local chain"),
        Err(e) => error!("Failed to add block to local chain: {}", e),
    }
}

impl NetworkBehaviourEventProcess<FloodsubEvent> for BlockchainBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        if let FloodsubEvent::Message(msg) = event {
            if let Ok(resp) = serde_json::from_slice::<ChainResponse>(&msg.data) {
                if resp.receiver == PEER_ID.to_string() {
                    handle_chain_response(&msg, resp, &mut self.blockchain);
                }
            }

            if let Ok(_) = serde_json::from_slice::<LocalChainRequest>(&msg.data) {
                handle_local_chain_request(&msg, &self.blockchain, &mut self.response_sender);
            }

            if let Ok(block) = serde_json::from_slice::<Block>(&msg.data) {
                handle_received_block(block, &mut self.blockchain);
            }
        }
    }
}

pub fn get_list_of_peers(swarm: &Swarm<BlockchainBehaviour>) -> Vec<String> {
    info!("Discovered peers:");

    let nodes = swarm.behaviour().mdns.discovered_nodes();
    let mut unique_peers = HashSet::new();

    nodes.into_iter().for_each(|peer| {
        unique_peers.insert(peer);
    });

    unique_peers
        .iter()
        .map(|peer| peer.to_string())
        .collect()
}

pub fn handle_print_peers(swarm: &Swarm<BlockchainBehaviour>) {
    get_list_of_peers(swarm)
        .iter()
        .for_each(|peer| info!("{}", peer));
}

pub fn handle_print_chain(swarm: &Swarm<BlockchainBehaviour>) {
    info!("Local blockchain:");

    let pretty_json = serde_json
        ::to_string_pretty(&swarm.behaviour().blockchain.blocks)
        .expect("can parse blocks");

    info!("{}", pretty_json);
}

pub fn handle_create_block(cmd: &str, swarm: &mut Swarm<BlockchainBehaviour>) {
    cmd.strip_prefix("create block").and_then(|data| {
        let behaviour = swarm.behaviour_mut();

        let latest_block = behaviour.blockchain.blocks.last().unwrap();

        let new_block = Block::new(
            latest_block.header.id + 1,
            &latest_block.header.hash,
            data.to_owned()
        );

        let stringified_new_block = new_block.to_json_string().expect("can stringify block");

        behaviour.blockchain.blocks.push(new_block);

        info!("Broadcasting block to peers...");

        behaviour.floodsub.publish(BLOCK_TOPIC.clone(), stringified_new_block.as_bytes());

        Some(())
    });
}
