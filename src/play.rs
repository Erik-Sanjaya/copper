use crate::ProtocolError;

#[derive(Debug)]
pub struct ServerBound;
#[derive(Debug)]
pub struct ClientBound;

impl ClientBound {
    pub fn from_request(request: ServerBound) -> Result<Self, ProtocolError> {
        Err(ProtocolError::Unimplemented)
    }
}
