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
        while (self.connected) {
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

            // trace!("BUFFER {:?}", self.buffer);

            // if !self.buffer.is_empty() {
            //     trace!("BUFFER NOT EMPTY");
            //     match self.read_stream().await {
            //         Ok(packet) => match packet {
            //             ServerBound::Handshake(_req) => {
            //                 trace!("Handshake Packet Incoming {:?}", _req);
            //             }
            //             ServerBound::Status(req) => {
            //                 trace!("Status Packet Incoming {:?}", req);
            //                 let reply_packet = ClientBound::parse_packet(
            //                     // &mut self.stream,
            //                     &self.state,
            //                     ServerBound::Status(req),
            //                 )
            //                 .unwrap();

            //                 let bytes_written =
            //                     reply_packet.write_to(&mut self.stream).await.unwrap();

            //                 // self.buffer.drain(0..bytes_read);
            //                 // self.buffer.drain(0..)

            //                 trace!("Status Packet Written");
            //             }

            //             ServerBound::Login(_req) => unimplemented!(),
            //             ServerBound::Play(_req) => unimplemented!(),
            //         },
            //         Err(_e) => panic!(),
            //     }
            // }
        }
    }

    async fn reply(&mut self) {
        // while self.buffer.is_empty() {
        //     trace!("YIELD REPLY");
        //     tokio::task::yield_now().await;
        // }

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
                trace!("Handshake Packet Incoming {:?}", req);
                if let HandshakingServerBound::Handshake(handshake) = req {
                    let next_state = handshake.get_next_state();
                    match next_state {
                        HandshakingNextState::Status => self.state = State::Status,
                        HandshakingNextState::Login => self.state = State::Login,
                    }
                }

                // self.state =
            }
            ServerBound::Status(req) => {
                trace!("Status Packet Incoming {:?}", req);
                let reply_packet = ClientBound::create_reply(
                    // &mut self.stream,
                    &self.state,
                    ServerBound::Status(req),
                )
                .unwrap();

                // check if it's a ping response
                // client doesn't do anything else after this so it's safe to terminate
                if let ClientBound::Status(StatusClientBound::PingResponse(_)) = reply_packet {
                    self.connected = false;
                }

                let reply_bytes = reply_packet.encode().unwrap();

                // let bytes_written = reply_packet.write_to(&mut self.stream).await.unwrap();
                let bytes_written = self.stream.try_write(&reply_bytes[..]).unwrap();

                debug!("Status packet written");
                trace!("Packet: {:?}", reply_bytes);
            }

            ServerBound::Login(_req) => unimplemented!(),
            ServerBound::Play(_req) => unimplemented!(),
        };
    }

    // async fn parse_packet(&mut self) -> Result<ServerBound, ProtocolError> {
    //     let mut length_buffer = [0; 5];

    //     // honestly, this peeking thing is only done because of the legacy ping
    //     // server list that has no length at all.
    //     // i don't like this double read on the length and have it be unused, so
    //     // i'll probably just hard code it here later.
    //     self.stream.peek(&mut length_buffer[..]).await;
    //     let mut length_cursor = Cursor::new(length_buffer.as_slice());
    //     let length = VarInt::read_from(&mut length_cursor)?;
    //     let mut packet = vec![0; length.size() + length.0 as usize];

    //     self.stream.read_exact(&mut packet).await?;

    //     let mut cursor = Cursor::new(packet.as_slice());
    //     ServerBound::parse_packet(&mut cursor, &self.state)
    // }

    // fn parse_packet(&self, packet: &[u8]) -> Result<ServerBound, ProtocolError> {
    //     let mut cursor = Cursor::new(packet);
    //     ServerBound::parse_packet(&mut cursor, &self.state)
    // }

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
                Ok(0) => break,
                Ok(bytes_read) => {
                    trace!("BYTES READ: {:?}", bytes_read);
                    if bytes_read == 0 {
                        break; // Stream is closed
                    }

                    self.buffer.extend_from_slice(&buffer[..bytes_read]);
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, yield to other tasks
                    // trace!("YIELD");
                    // tokio::task::yield_now();
                    trace!("BREAK DRAIN");
                    break;
                }
                Err(err) => return Err(err), // Forward other errors
            }
        }

        Ok(())
    }
}
