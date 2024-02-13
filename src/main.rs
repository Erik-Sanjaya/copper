mod client;
mod data_types;
mod handshaking;
mod login;
mod packet;
mod play;
mod server_status;
mod status;

use std::{net::SocketAddr, ops::Index};

use thiserror::Error;
use tokio::sync::mpsc::channel;
use tracing::{error, info, trace};

#[derive(Debug, Error)]
pub enum ProtocolError {
    /// There's no packet id that matches the one given
    #[error("Packet id doesn't have the type: {0}")]
    PacketId(i32),
    /// Usually when parsing stuff, if there's a case of missing bytes, it should give back this error
    #[error("Missing data")]
    Missing,
    /// When the parsing simply fails or have unexpected value
    #[error("Malformed data")]
    Malformed,
    #[error("IO error")]
    /// Any error coming from std::io::Error
    IOError(#[source] std::io::Error),
    /// For features that have not been implemented yet.
    #[error("Unimplemented")]
    Unimplemented,
    #[error("Parsing error")]
    Parsing,
    #[error("serde_json error")]
    SerdeJson(#[source] serde_json::error::Error),
    #[error("Internal error")]
    Internal,
    #[error("TryFromInt error")]
    TryFromInt(#[source] std::num::TryFromIntError),
}

impl From<std::io::Error> for ProtocolError {
    fn from(error: std::io::Error) -> Self {
        Self::IOError(error)
    }
}

impl From<serde_json::error::Error> for ProtocolError {
    fn from(error: serde_json::error::Error) -> Self {
        Self::SerdeJson(error)
    }
}

impl From<std::num::TryFromIntError> for ProtocolError {
    fn from(error: std::num::TryFromIntError) -> Self {
        Self::TryFromInt(error)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Handshaking,
    Status,
    Login,
    Play,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        // .pretty()
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    // let listener = TcpListener::bind("127.0.0.1:25565").await?;
    // while let ok((stream, addr)) = listener.accept().await {
    //     info!("Connection made with {addr}");
    //     trace!("{stream:?}");
    //     // let mut client = client::Client::new(stream, addr);
    //     // client.handle();
    // }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:25565").await?;
    let mut clients = vec![];
    let (tx, mut rx) = tokio::sync::mpsc::channel::<SocketAddr>(32);

    loop {
        tokio::select! {
            res = listener.accept() => {
                match res {
                    Ok((stream, addr)) => {
                        clients.push(addr);
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            info!("Client ({addr}) has connected.");
                            client::Client::new(stream, addr, tx).handle().await;
                        })
                        .await?;
                    },
                    Err(e) => {
                        error!("{e:?}");

                    }
                }
            }

            disconnect_addr = rx.recv() => {
                let disconnect_addr = disconnect_addr.expect("recv fail");
                info!("Client ({disconnect_addr}) has disconencted.");
                clients.swap_remove(clients.iter().position(|a| *a == disconnect_addr).expect("addr should be inside, unless its disconnected"));
            }
        }
        trace!("List of clients: {clients:?}");
    }
}
