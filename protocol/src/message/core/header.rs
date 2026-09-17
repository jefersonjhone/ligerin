use crate::core::{error::ParseError, method::Method, resource::Resource, status::Status};

/// Identificador do protocolo na start-line.
pub const PROTOCOL: &str = "LPC";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestHeader {
    pub protocol: String,
    pub resource: Resource,
    pub message_size: u32,
    pub method: Method,
}

impl RequestHeader {
    pub fn new(protocol: String, resource: Resource, message_size: u32, method: Method) -> Self {
        Self { protocol, resource, message_size, method }
    }

    /// Parseia a start-line: `LPC <METHOD> <resource> <message_size>`.
    pub fn parse(line: &str) -> Result<Self, ParseError> {
        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            [PROTOCOL, method_str, resource_str, size_str] => {
                let method = method_str.parse::<Method>().map_err(|_| {
                    ParseError::invalid_header("method", "[GET, POST]", *method_str)
                })?;
                let resource = Resource::parse(resource_str)?;
                let message_size = size_str.parse::<u32>().map_err(|_| {
                    ParseError::invalid_header("message_size", "número inteiro (u32)", *size_str)
                })?;
                Ok(Self::new(PROTOCOL.to_string(), resource, message_size, method))
            }
            tokens => Err(ParseError::invalid_header(
                "start-line",
                "LPC <METHOD> <resource> <message_size>",
                tokens.join(" "),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseHeader {
    pub protocol: String,
    pub resource: Resource,
    pub message_size: u32,
    pub status: Status,
}

impl ResponseHeader {
    pub fn new(protocol: String, resource: Resource, message_size: u32, status: Status) -> Self {
        Self { protocol, resource, message_size, status }
    }

    /// Parseia a start-line: `LPC <resource> <STATUS> <message_size>`.
    pub fn parse(line: &str) -> Result<Self, ParseError> {
        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            [PROTOCOL, resource_str, status_str, size_str] => {
                let resource = Resource::parse(resource_str)?;
                let code = status_str.parse::<u16>().map_err(|_| {
                    ParseError::invalid_header("status", "código numérico (u16)", *status_str)
                })?;
                let status = Status::try_from(code).map_err(|code| {
                    ParseError::invalid_header(
                        "status",
                        "código conhecido (200, 201, 400, 401, 403, 404, 409, 500)",
                        code.to_string(),
                    )
                })?;
                let message_size = size_str.parse::<u32>().map_err(|_| {
                    ParseError::invalid_header("message_size", "número inteiro (u32)", *size_str)
                })?;
                Ok(Self::new(PROTOCOL.to_string(), resource, message_size, status))
            }
            tokens => Err(ParseError::invalid_header(
                "start-line",
                "LPC <resource> <STATUS> <message_size>",
                tokens.join(" "),
            )),
        }
    }
}