use std::error::Error;
use std::fmt;

/// Erro ao decodificar/validar uma mensagem LPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Start-line inválida (`LPC <METHOD> <resource> <message_size>`).
    InvalidHeader { field: &'static str, expected: String, found: String },
    /// Linha de metadados inválida (`chave: valor`).
    InvalidMetadata { field: &'static str, expected: String, found: String },
    /// Payload inválido (tamanho divergente de `message_size`, bytes inválidos).
    InvalidPayload { field: &'static str, expected: String, found: String },
}

impl ParseError {
    pub fn invalid_header(
        field: &'static str,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        Self::InvalidHeader { field, expected: expected.into(), found: found.into() }
    }

    pub fn invalid_metadata(
        field: &'static str,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        Self::InvalidMetadata { field, expected: expected.into(), found: found.into() }
    }

    pub fn invalid_payload(
        field: &'static str,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        Self::InvalidPayload { field, expected: expected.into(), found: found.into() }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader { field, expected, found } => write!(
                f,
                "start-line inválida: campo '{field}', esperado '{expected}', encontrado '{found}'"
            ),
            Self::InvalidMetadata { field, expected, found } => write!(
                f,
                "metadado inválido: campo '{field}', esperado '{expected}', encontrado '{found}'"
            ),
            Self::InvalidPayload { field, expected, found } => write!(
                f,
                "payload inválido: campo '{field}', esperado '{expected}', encontrado '{found}'"
            ),
        }
    }
}

impl Error for ParseError {}