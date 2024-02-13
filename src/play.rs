use crate::ProtocolError;

#[derive(Debug)]
pub enum ServerBound {
    BundleDelimiter(BundleDelimiter),
}

#[derive(Debug)]
struct BundleDelimiter;

#[derive(Debug)]
pub enum ClientBound {}

impl ClientBound {
    pub fn from_request(request: ServerBound) -> Result<Self, ProtocolError> {
        Err(ProtocolError::Unimplemented)
    }
}
