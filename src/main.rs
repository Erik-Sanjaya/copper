mod data_types;
mod handshaking;
mod login;
mod packet;
mod server_status;
mod status;

use std::{
    io::{Cursor, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
};

use anyhow::anyhow;
use data_types::VarInt;
use serde_json::json;
use status::Status;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

use crate::handshaking::{Handshaking, HandshakingNextState};

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
}
fn stream_into_vec(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    // TODO: handle legacy server ping list https://wiki.vg/Server_List_Ping#1.6
    // cancer part is that it doesn't have a length prefixed. actually breaking
    // protocol
    let length = VarInt::read_from(stream)?;
    if length.0 == 0 {
        trace!("length is 0, most likely EOF");
        return Err(anyhow!("length is 0"));
    }

    let mut buffer = vec![0; length.0 as usize];
    match stream.read_exact(&mut buffer[..]) {
        Ok(_) => (),
        Err(e) => {
            error!("Error with reading_exact: {:?}", e);
            debug!("Buffer length: {:?}", length);
            debug!("Buffer: {:?}", buffer);
            return Err(e.into());
        }
    };

    Ok(buffer)
}

#[derive(Debug, PartialEq)]
enum State {
    Handshaking,
    Status,
    Login,
    Play,
}

fn handle_handshaking(
    cursor: &mut Cursor<&[u8]>,
    state: &mut State,
) -> anyhow::Result<Handshaking> {
    let handshake = Handshaking::read(cursor)?;
    debug!("{:?}", handshake);

    match handshake.next_state {
        HandshakingNextState::Status => *state = State::Status,
        HandshakingNextState::Login => *state = State::Login,
    }

    Ok(handshake)
}

fn handle_status(cursor: &mut Cursor<&[u8]>, writer: &mut TcpStream) -> anyhow::Result<Status> {
    let status = Status::read(cursor)?;
    status.write(writer)?;

    Ok(status)
}

fn handle_login(
    cursor: &mut Cursor<&[u8]>,
    writer: &mut TcpStream,
    state: &mut State,
) -> anyhow::Result<()> {
    // TODO: implement all of these into actual struct.
    let mut reply: Vec<u8> = vec![];

    let reply_json = json!({
        "text": "WIP",

    })
    .to_string();

    let packet_id = VarInt(0);
    let string_len = VarInt(reply_json.len() as i32);
    let packet_len = VarInt((packet_id.size() + string_len.size() + reply_json.len()) as i32);

    packet_len.write_to(&mut reply);
    packet_id.write_to(&mut reply);

    string_len.write_to(&mut reply);
    reply.extend_from_slice(reply_json.as_bytes());

    writer.write_all(&reply)?;
    info!("WRITE LOGIN REPLY");
    writer.shutdown(Shutdown::Both)?;

    Ok(())
}

fn handle_socket(mut stream: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
    let mut state = State::Handshaking;

    loop {
        trace!("State of {}: {:?}", addr, state);
        let buffer = stream_into_vec(&mut stream)?;
        debug!("Buffer from {}: {:?}", addr, buffer);
        let mut cursor = Cursor::new(buffer.as_slice());

        match state {
            State::Handshaking => {
                let handshake = handle_handshaking(&mut cursor, &mut state)?;
                info!("Handshake processed from {}", addr);
                trace!("{} | {:?}", addr, handshake);
            }
            State::Status => {
                let status = handle_status(&mut cursor, &mut stream)?;
                info!("Status processed from {}", addr);
                trace!("{} | {:?}", addr, status);
            }
            State::Login => {
                let login = handle_login(&mut cursor, &mut stream, &mut state)?;
                info!("Login processed from {}", addr);
                trace!("{} | {:?}", addr, login);
            }
            State::Play => {
                info!("ENTERING PLAY STATE");
            }
        };
    }
}

fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        // .pretty()
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    let listener = TcpListener::bind("127.0.0.1:25565")?;
    while let Ok((stream, addr)) = listener.accept() {
        info!("Connection made with {addr}");
        trace!("{stream:?}");
        match handle_socket(stream, addr) {
            Ok(_) => (),
            Err(e) => warn!("Socket handling failed: {e}"),
        };
    }

    // let listener = TcpListener::bind("127.0.0.1:25565").await?;
    // loop {
    //     let (stream, _addr) = listener.accept().await?;
    //     tokio::spawn(async move {
    //         info!("{:?}", stream);
    //         let _ = handle_socket(stream)
    //             .await
    //             .map_err(|err| error!("{:?}", err));
    //     })
    //     .await?;
    // }

    Ok(())
}
