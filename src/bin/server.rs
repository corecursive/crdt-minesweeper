use std::{
    collections::HashMap,
    net::{IpAddr, Ipv6Addr}, future,
};

use automerge::{
    sync::{Message, State, SyncDoc},
    AutoCommit,
};
use autosurgeon::reconcile;

use crdt_minesweeper::{Grid, MineField, Rpc, FIELD_SIZE};
use futures::StreamExt;
use tarpc::{
    context,
    server::{self, incoming::Incoming, Channel},
    tokio_serde::formats::Bincode,
};
use tokio::sync::mpsc::{self, Sender, Receiver};

#[derive(Debug)]
struct Error;

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }
}

// This is the type that implements the generated World trait. It is the business logic
// and is used to start the server.
#[derive(Clone)]
pub struct GameState {
    peer: AutoCommit,
    peer_state: State,
}

// This struct implements the RPC handlers
pub struct GameServer {
    receiver: Receiver<SyncRequest>,
    clients: HashMap<ClientId, Client>,
}

#[derive(Clone)]
struct MessageQueue(Vec<Vec<u8>>);

struct SyncRequest {
    client_id: ClientId,
    messages: HashMap<ClientId, MessageQueue>
}

struct GameServerConnection {
    sender: Sender<SyncRequest>,
    receiver: Receiver<SyncRequest>,
}

// FIXME receiver does not implement clone, so we cannot just derive?
impl Clone for GameServerConnection {
    fn clone(&self) -> Self {
        GameServerConnection {
            sender: self.sender.clone(),
            receiver: self.receiver,
        }
    }
}

struct SyncResponse {
    messages: HashMap<ClientId, MessageQueue>
}

#[derive(Clone)]
struct ClientId(String);

#[derive(Clone)]
struct Client {
    id: ClientId,
    sender: Sender<SyncResponse>,
    messages: HashMap<ClientId, MessageQueue>,
}

#[tarpc::server]
impl Rpc for GameServerConnection {
    // Each defined rpc generates two items in the trait, a fn that serves the RPC, and
    // an associated type representing the future output by the fn.
    async fn sync(self, _: context::Context, message: Vec<u8>) -> Vec<u8> {
        if let Ok(message) = Message::decode(&message) {
            self.sender.send(message).await;
            println!("decoded message");
            Vec::new()
        } else {
            Vec::new()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut peer1 = automerge::AutoCommit::new();
    let mut peer1_state = automerge::sync::State::new();
    // Peer 1 puts data into the document
    reconcile(
        &mut peer1,
        &MineField {
            grid: Grid::new(FIELD_SIZE),
        },
    )
    .unwrap();
    let _message1to2 = peer1
        .sync()
        .generate_sync_message(&mut peer1_state)
        .ok_or(Error {})
        .unwrap()
        .encode();

    let server_addr = (IpAddr::V6(Ipv6Addr::LOCALHOST), 6009);

    let (tx, rx) = mpsc::channel(3);

    let server = GameServer {
        receiver: todo!(),
        clients: HashMap::new(),
    };

    let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Bincode::default).await?;
    listener.config_mut().max_frame_length(usize::MAX);
    listener
        // Ignore accept errors.
        .filter_map(|r| future::ready(r.ok()))
        .map(server::BaseChannel::with_defaults)
        // Limit channels to 1 per IP.
        .max_channels_per_key(1, |t| t.transport().peer_addr().unwrap().ip())
        // serve is generated by the service attribute. It takes as input any type implementing
        // the generated World trait.
        .map(|channel| {
            let rpc_server = GameServerConnection {
                receiver: rx,
                sender: tx.clone(),
            };
            channel.execute(rpc_server.serve())
        })
        // Max 10 channels.
        .buffer_unordered(10)
        .for_each(|_| async {
            // act on channels here if necessary
        })
        .await;

    // stream.write(&message1to2);

    Ok(())
}
