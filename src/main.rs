mod data_types;
mod handshaking;
mod login;
mod packet;
mod server_status;
mod status;

use std::{
    io::{Cursor, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
};

use anyhow::anyhow;
use data_types::{DataType, VarInt};
use packet::ServerBound;

use status::Status;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

use crate::{
    handshaking::{Handshaking, HandshakingNextState},
    packet::ClientBound,
};

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

fn stream_into_vec(stream: &mut TcpStream) -> Result<Vec<u8>, ProtocolError> {
    // TODO: handle legacy server ping list https://wiki.vg/Server_List_Ping#1.6
    // cancer part is that it doesn't have a length prefixed. actually breaking
    // protocol
    let length = VarInt::read_from(stream)?;

    if length.0 == 0 {
        trace!("length is 0, most likely EOF but shouldn't be possible");
        panic!("EOF check should've been made in VarInt::read_from");
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
pub enum State {
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
    trace!("reading request");
    let request = login::ServerBound::read_from(cursor)?;
    let mut reply_buffer: Vec<u8> = vec![];

    trace!("writing response");
    match request {
        login::ServerBound::LoginStart(packet) => {
            let response_packet = login::LoginSuccess {
                uuid: packet.player_uuid.unwrap(),
                username: packet.name,
                number_of_properties: VarInt(0),
                name: None,
                value: None,
                is_signed: None,
                signature: None,
            };

            login::ClientBound::LoginSuccess(response_packet).write_to(&mut reply_buffer)?;
            *state = State::Play;
        }
        login::ServerBound::EncryptionResponse(_) => {
            return Err(anyhow!("UNIMPLEMENTED"));
        }
        login::ServerBound::LoginPluginResponse(_) => {
            return Err(anyhow!("UNIMPLEMENTED"));
        }
    };

    // let reply_json = ProtocolString::from(
    //     json!({
    //         "text": "WIP",

    //     })
    //     .to_string(),
    // );

    // let reply_packet = login::ClientBound::Disconnect(login::Disconnect::new(reply_json));
    // reply_packet.write_to(&mut reply_buffer)?;

    trace!("write to stream");
    writer.write_all(&reply_buffer)?;
    info!("WRITE LOGIN REPLY");
    writer.shutdown(Shutdown::Both)?;

    Ok(())
}

fn handle_socket(mut stream: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
    let mut state = State::Handshaking;
    let (server_packet_sender, server_packet_receiver) = mpsc::channel::<ServerBound>();

    loop {
        trace!("State of {}: {:?}", addr, state);
        // let buffer = match stream_into_vec(&mut stream) {
        //     Ok(buf) => buf,
        //     Err(e) => {
        //         warn!("error while getting buffer: {e}");
        //         vec![]
        //     }
        // };
        let buffer = stream_into_vec(&mut stream)?;
        debug!("Buffer from {}: {:?}", addr, buffer);
        let mut cursor = Cursor::new(buffer.as_slice());

        server_packet_sender.send(ServerBound::parse_packet(&mut stream, &state)?)?;
        match server_packet_receiver.recv() {
            Ok(packet) => {
                ClientBound::parse_packet(&mut stream, &state, packet)?.write_to(&mut stream)?;
            }
            Err(e) => {
                trace!("channel is probably empty {e:?}");
            }
        };

        // match state {
        //     State::Handshaking => {
        //         let handshake = handle_handshaking(&mut cursor, &mut state)?;
        //         info!("Handshake processed from {}", addr);
        //         trace!("{} | {:?}", addr, handshake);
        //     }
        //     State::Status => {
        //         let status = handle_status(&mut cursor, &mut stream)?;
        //         info!("Status processed from {}", addr);
        //         trace!("{} | {:?}", addr, status);
        //     }
        //     State::Login => {
        //         let login = handle_login(&mut cursor, &mut stream, &mut state)?;
        //         info!("Login processed from {}", addr);
        //         trace!("{} | {:?}", addr, login);
        //     }
        //     State::Play => {
        //         info!("ENTERING PLAY STATE");
        //     }
        // };
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
