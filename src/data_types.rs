use std::io::Cursor;

use thiserror::Error;
use tokio::io::AsyncReadExt;
use tracing::error;

#[derive(Debug, Error)]
pub enum VarIntError {
    #[error("Bytes exceeded the limit of VarInt")]
    Overflow,
    #[error("Bytes too short")]
    MissingBytes,
}

#[derive(Debug)]
pub struct VarInt(pub i32);

impl VarInt {
    pub async fn read(cursor: &mut Cursor<&[u8]>) -> Result<Self, VarIntError> {
        let mut result = 0;
        let mut shift = 0;

        loop {
            if shift >= 32 {
                return Err(VarIntError::Overflow);
            }

            let byte = match cursor.read_u8().await {
                Ok(b) => b,
                Err(e) => {
                    error!("{:?}", e);
                    return Err(VarIntError::MissingBytes);
                }
            };

            result |= ((byte & 0x7F) as i32) << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                break;
            }
        }

        Ok(Self(result))
    }
}
