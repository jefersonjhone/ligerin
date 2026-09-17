/// Corpo da mensagem — um documento JSON (texto UTF-8). 
/// O payload não interpreta o conteúdo: trafega apenas bytes, quem consome decide o formato.
/// 
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Payload {
    bytes: Vec<u8>,
}

impl Payload {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn from_str(text: impl AsRef<str>) -> Self {
        Self { bytes: text.as_ref().as_bytes().to_vec() }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}