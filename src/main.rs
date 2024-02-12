mod client;
mod data_types;
mod handshaking;
mod login;
mod packet;
mod play;
mod server_status;
mod status;

use thiserror::Error;
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
}

impl From<std::io::Error> for ProtocolError {
    fn from(error: std::io::Error) -> Self {
        ProtocolError::IOError(error)
    }
}

impl From<serde_json::error::Error> for ProtocolError {
    fn from(error: serde_json::error::Error) -> Self {
        ProtocolError::SerdeJson(error)
    }
}

#[derive(Debug, PartialEq)]
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
    loop {
        let (stream, addr) = listener.accept().await?;
        tokio::spawn(async move {
            info!("Client connected from: {:?}", addr);
            trace!("Client Stream: {:?}", stream);
            client::Client::new(stream, addr).handle().await;
        })
        .await?;
    }

    Ok(())
}
