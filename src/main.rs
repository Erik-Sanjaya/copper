mod data_types;
mod handshaking;
mod status;

use std::{
    io::{Cursor, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    time::Duration,
};

use data_types::{VarInt, VarIntError};
use serde_json::json;
use status::Status;
use thiserror::Error;
use tracing::{debug, error, info, instrument::WithSubscriber, trace, warn};

use crate::{
    handshaking::{Handshaking, HandshakingNextState},
    status::StatusPacketId,
};

fn stream_into_vec(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let mut length_buffer = [0; 5];
    stream.peek(&mut length_buffer[..])?;

    let mut length_cursor = Cursor::new(&length_buffer[..]);
    let length = VarInt::read(&mut length_cursor)?;

    let mut buffer = vec![0; length.0 as usize + length_cursor.position() as usize];
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

#[derive(Debug)]
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

    packet_len.write(&mut reply);
    packet_id.write(&mut reply);

    string_len.write(&mut reply);
    reply.extend_from_slice(reply_json.as_bytes());

    writer.write_all(&reply)?;
    info!("WRITE LOGIN REPLY");
    writer.shutdown(Shutdown::Both)?;

    Ok(())
}

fn handle_socket(mut stream: TcpStream) -> anyhow::Result<()> {
    let mut state = State::Handshaking;

    loop {
        trace!("state: {:?}", state);
        let buffer = stream_into_vec(&mut stream)?;

        debug!("Buffer from stream: {:?}", buffer);

        let mut cursor = Cursor::new(buffer.as_slice());

        match state {
            State::Handshaking => {
                let handshake = handle_handshaking(&mut cursor, &mut state)?;
                info!("Handshake processed: {:?}", handshake);
            }
            State::Status => {
                let status = handle_status(&mut cursor, &mut stream)?;
                info!("Status processed: {:?}", status);
            }
            State::Login => {
                let _ = handle_login(&mut cursor, &mut stream, &mut state)?;
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
        match handle_socket(stream) {
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
