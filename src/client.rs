use std::{io::Cursor, net::SocketAddr};

use tokio::net::TcpStream;
use tracing::debug;
use tracing::error;
use tracing::trace;

use crate::handshaking::HandshakingNextState;
use crate::handshaking::HandshakingServerBound;
use crate::packet::ClientBound;
use crate::packet::ServerBound;
use crate::status::Status;
use crate::status::StatusClientBound;
use crate::ProtocolError;
use crate::State;

pub struct Client {
    addr: SocketAddr,
    stream: TcpStream,
    state: State,
    buffer: Vec<u8>,
    connected: bool,
}

impl Client {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Self {
            addr,
            stream,
            state: State::Handshaking,
            buffer: Vec::new(),
            connected: true,
        }
    }

    // i think this one is allowed to consume the client
    // all the calls to the other methods come from this
    // one anyway
    pub async fn handle(&mut self) {
        while self.connected {
            // basically, have the thing be drained
            // then it should be yielding once it empties out.
            // at this stage you already be writing a response packet to the client
            // that way the program wont just keep on yielding the drain
            // and have nothing else to do

            trace!("draining stream");
            self.drain_stream().await.unwrap();

            // tokio::select! {
            // _ = self.drain_stream() => {
            //    // self.read_stream().await;
            // }
            // }

            trace!("replying packets");
            self.reply().await;
        }
    }

    async fn reply(&mut self) {
        if self.buffer.is_empty() {
            trace!("buffer is empty");
            return;
        }

        let (packet, bytes_read) = match self.read_stream().await {
            Ok(item) => item,
            Err(e) => {
                error!("{:?}", e);
                panic!();
            }
        };

        self.buffer.drain(0..bytes_read);

        match packet {
            ServerBound::Handshake(req) => {
                trace!("Handshake Packet Incoming: {:?}", req);
                if let HandshakingServerBound::Handshake(handshake) = req {
                    let next_state = handshake.get_next_state();
                    match next_state {
                        HandshakingNextState::Status => self.state = State::Status,
                        HandshakingNextState::Login => self.state = State::Login,
                    }
                }
            }
            ServerBound::Status(req) => {
                trace!("Status Packet Incoming: {:?}", req);
                let reply_packet =
                    ClientBound::create_reply(&self.state, ServerBound::Status(req)).unwrap();

                trace!("Status reply packet: {:?}", reply_packet);

                // check if it's a ping response
                // client doesn't do anything else after this so it's safe to terminate
                if let ClientBound::Status(StatusClientBound::PingResponse(_)) = reply_packet {
                    self.connected = false;
                }

                let reply_bytes = reply_packet.encode().unwrap();

                let bytes_written = self.stream.try_write(&reply_bytes).unwrap();

                debug!("Status packet written");
                trace!("Packet bytes: {:?}", reply_bytes);
            }

            ServerBound::Login(req) => {
                trace!("Login Packet Incoming: {:?}", req);
                let reply_packet =
                    ClientBound::create_reply(&self.state, ServerBound::Login(req)).unwrap();

                trace!("Login reply packet: {:?}", reply_packet);

                let reply_bytes = reply_packet.encode().unwrap();

                let bytes_written = self.stream.try_write(&reply_bytes).unwrap();

                debug!("Login packet written");
                trace!("Packet bytes: {:?}", reply_bytes);
            }
            ServerBound::Play(_req) => unimplemented!(),
        };
    }

    pub async fn read_stream(&mut self) -> Result<(ServerBound, usize), ProtocolError> {
        let mut temp_cursor = Cursor::new(&self.buffer);

        Ok((
            ServerBound::parse_packet(&mut temp_cursor, &self.state)?,
            temp_cursor.position() as usize,
        ))
    }

    /// Drain the whole stream and move it to the client's internal buffer
    async fn drain_stream(&mut self) -> Result<(), std::io::Error> {
        loop {
            let mut buffer = [0; 128];

            match self.stream.try_read(&mut buffer) {
                Ok(bytes_read) => {
                    trace!("Bytes read: {:?}", bytes_read);
                    if bytes_read == 0 {
                        self.connected = false; // Stream is closed
                        break;
                    }

                    self.buffer.extend_from_slice(&buffer[..bytes_read]);
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    trace!("would block, break");
                    break;
                }
                Err(err) => return Err(err), // Forward other errors
            }
        }

        Ok(())
    }
}
