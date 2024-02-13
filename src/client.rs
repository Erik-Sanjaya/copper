use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::SocketAddr;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;

use crate::handshaking;
use crate::login;
use crate::packet;
use crate::status;

use crate::State;

pub struct Client {
    addr: SocketAddr,
    stream: BufReader<TcpStream>,
    state: State,
    buffer: VecDeque<u8>,
    connected: bool,
    packet_queue: VecDeque<packet::ClientBound>,
    disconnect_tx: tokio::sync::mpsc::Sender<SocketAddr>,
}

impl Client {
    pub fn new(
        stream: TcpStream,
        addr: SocketAddr,
        tx: tokio::sync::mpsc::Sender<SocketAddr>,
    ) -> Self {
        Self {
            addr,
            stream: BufReader::new(stream),
            state: State::Handshaking,
            // vecdeque because the bytes are one time read only, so it's fine if its not contiguous for cache hit
            buffer: VecDeque::new(),
            connected: true,
            packet_queue: VecDeque::new(),
            disconnect_tx: tx,
        }
    }

    // i think this one is allowed to consume the client
    // all the calls to the other methods come from this
    // one anyway
    pub async fn handle(&mut self) {
        trace!("Client Stream: {:?}", self.stream);

        while self.connected {
            // basically, have the thing be drained
            // then it should be yielding once it empties out.
            // at this stage you already be writing a response packet to the client
            // that way the program wont just keep on yielding the drain
            // and have nothing else to do

            if let Err(e) = self.drain_stream().await {
                if e.kind() != ErrorKind::WouldBlock {
                    error!("Error from draining stream: {e:?}");
                    panic!("Error from draining stream: {e:?}");
                }
            }

            while !self.buffer.is_empty() {
                let packet = match packet::ServerBound::parse_packet(&mut self.buffer, &self.state)
                {
                    Ok(packet) => packet,
                    Err(e) => {
                        error!("{:?}", e);
                        panic!();
                    }
                };

                self.create_reply(packet);
            }

            while !self.packet_queue.is_empty() {
                self.write_packet().await;
            }
        }

        // client has disconnected
        self.disconnect_tx.send(self.addr).await;
    }

    /// Create packet(s) and then push it to `self.packet_queue`
    fn create_reply(&mut self, packet_to_write: packet::ServerBound) {
        let reply_packet: Option<packet::ClientBound> = match packet_to_write {
            packet::ServerBound::Handshake(req) => {
                info!("Handshake Packet Incoming: {:?}", req);
                if let handshaking::ServerBound::Handshake(handshake) = req {
                    self.state = handshake.get_next_state();
                }

                None
            }
            packet::ServerBound::Status(req) => {
                info!("Status Packet Incoming: {:?}", req);
                let reply_packet = packet::ClientBound::create_reply(
                    &self.state,
                    packet::ServerBound::Status(req),
                );

                let reply_packet = match reply_packet {
                    Ok(packet) => packet,
                    Err(e) => {
                        error!("Error in status packet reply: {e:?}");
                        todo!("complete error handling");
                    }
                };

                info!("Status reply packet: {reply_packet:?}");

                // check if it's a ping response
                // client doesn't do anything else after this so it's safe to terminate
                if let packet::ClientBound::Status(status::ClientBound::PingResponse(_)) =
                    reply_packet
                {
                    self.connected = false;
                }

                Some(reply_packet)
            }

            packet::ServerBound::Login(req) => {
                info!("Login Packet Incoming: {:?}", req);
                let reply_packet =
                    packet::ClientBound::create_reply(&self.state, packet::ServerBound::Login(req));

                let reply_packet = match reply_packet {
                    Ok(packet) => packet,
                    Err(e) => {
                        error!("Error in login packet reply: {e:?}");
                        todo!("complete error handling");
                    }
                };

                info!("Login reply packet: {reply_packet:?}");

                // check if it's a login success, in which you'd transition to the next state
                if let packet::ClientBound::Login(login::ClientBound::LoginSuccess(_)) =
                    reply_packet
                {
                    self.state = State::Play;
                }

                Some(reply_packet)
            }
            packet::ServerBound::Play(req) => {
                info!("Play Packet Incoming: {:?}", req);
                let reply_packet =
                    packet::ClientBound::create_reply(&self.state, packet::ServerBound::Play(req));

                let reply_packet = match reply_packet {
                    Ok(packet) => packet,
                    Err(e) => {
                        error!("Error in login packet reply: {e:?}");
                        todo!("complete error handling");
                    }
                };

                Some(reply_packet)
            }
        };

        if let Some(reply_packet) = reply_packet {
            self.packet_queue.push_back(reply_packet);
        }
    }

    async fn write_packet(&mut self) {
        let reply_packet = self.packet_queue.pop_front();

        let Some(reply_packet) = reply_packet else {
            return;
        };

        let reply_bytes = match reply_packet.encode() {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Error in packet encoding: {e:?}");
                todo!("complete error handling");
            }
        };

        let bytes_written = match self.stream.write(&reply_bytes).await {
            Ok(bytes_written) => bytes_written,
            Err(e) => {
                error!("Error in try_write: {e:?}");
                todo!("complete error handling");
            }
        };

        debug!("Packet written {bytes_written} byte(s)");
        trace!("Packet bytes: {reply_bytes:?}");
    }

    /// Drain the whole stream and move it to the client's internal buffer
    async fn drain_stream(&mut self) -> Result<(), std::io::Error> {
        trace!("draining stream");
        let mut buffer: Vec<u8> = vec![0; 128];

        match self.stream.read(&mut buffer).await {
            Ok(bytes_read) => {
                trace!("Bytes read: {:?}", bytes_read);
                if bytes_read == 0 {
                    self.connected = false; // Stream is closed
                }

                self.buffer.extend(&buffer[..bytes_read]);
            }
            Err(err) => return Err(err), // Forward other errors
        }

        Ok(())
    }
}
