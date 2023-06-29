use std::io::Cursor;

use thiserror::Error;
use uuid::Uuid;

use crate::{
    data_types::{VarInt, VarIntError},
    status::PacketIdError,
};

// 1. C→S: Handshake with Next State set to 2 (login)
// 2. C→S: Login Start
// 3. S→C: Encryption Request
// 4. Client auth
// 5. C→S: Encryption Response
// 6. Server auth, both enable encryption
// 7. S→C: Set Compression (optional)
// 8. S→C: Login Success
pub struct LoginStart {
    name: String,
    has_player_uuid: bool,
    player_uuid: Option<Uuid>,
}

#[derive(Debug, Error)]
enum LoginStartError {
    #[error("Packet length is invalid: {0}")]
    Length(#[source] VarIntError),
    #[error("Error with packet id: {0}")]
    PacketId(#[source] VarIntError),
}

impl LoginStart {
    fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, LoginStartError> {
        let packet_id = VarInt::read_from(cursor).map_err(LoginStartError::PacketId)?;

        Ok(LoginStart {
            name: "".to_owned(),
            has_player_uuid: false,
            player_uuid: None,
        })
    }
}

struct EncryptionRequest {
    server_id: String,
    public_key_length: VarInt,
    public_key: Vec<u8>,
    verify_token_length: VarInt,
    verify_token: Vec<u8>,
}

struct EncryptionResponse {
    shared_secret_length: VarInt,
    shared_secret: Vec<u8>,
    verify_token_length: VarInt,
    verify_token: Vec<u8>,
}

struct SetCompression {
    threshold: VarInt,
}

struct LoginPluginResponse {
    message_id: VarInt,
    successful: bool,
    data: Option<Vec<u8>>,
}
