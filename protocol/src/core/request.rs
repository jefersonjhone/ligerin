use crate::core::error::ParseError;
use crate::core::method::Method;
use crate::message::core::{
    header::{RequestHeader, PROTOCOL},
    metadata::Metadata,
    payload::Payload,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub header: RequestHeader,
    pub metadata: Metadata,
    pub payload: Option<Payload>,
}

impl Request {
    pub fn new(header: RequestHeader, metadata: Metadata, payload: Option<Payload>) -> Self {
        Self { header, metadata, payload }
    }

    /// Decodifica uma requisição a partir da mensagem  recebida.
    ///
    /// ```text
    /// LPC <METHOD> <resource> <message_size>
    /// chave: valor            (0 ou mais linhas)
    ///                         (linha vazia termina o metadata)
    /// <payload: JSON, obrigatório em POST; message_size = total da mensagem>
    /// ```
    pub fn parse(message: &str) -> Result<Self, ParseError> {
        let mut parts = message.split_inclusive('\n');

        let header_line = parts.next().ok_or_else(|| {
            ParseError::invalid_header(
                "start-line",
                "LPC <METHOD> <resource> <message_size>",
                "<mensagem vazia>",
            )
        })?;
        let header = RequestHeader::parse(header_line.trim_end_matches('\n'))?;

        // message_size = tamanho TOTAL da mensagem (valida o frame inteiro)
        if message.len() != header.message_size as usize {
            return Err(ParseError::invalid_payload(
                "message_size",
                format!("{} bytes (total da mensagem)", header.message_size),
                format!("{} bytes", message.len()),
            ));
        }

        let metadata = Metadata::parse(&mut parts)?;

        let body: String = parts.collect();
        let payload = (!body.is_empty()).then(|| Payload::from_str(body));

        // payload é obrigatório em POST
        if header.method == Method::POST && payload.is_none() {
            return Err(ParseError::invalid_payload(
                "payload",
                "JSON obrigatório em POST",
                "<vazio>",
            ));
        }

        Ok(Request::new(header, metadata, payload))
    }


    /// O message_size é preenchido com um placeholder, depois prnchido com o valor real
    pub fn encode(&self) -> Result<Vec<u8>, ParseError> {
        let payload_bytes: &[u8] = self.payload.as_ref().map_or(&[], Payload::as_bytes);

        // payload é obrigatório em POST 
        if self.header.method == Method::POST && payload_bytes.is_empty() {
            return Err(ParseError::invalid_payload(
                "payload",
                "JSON obrigatório em POST",
                "<vazio>",
            ));
        }

        let mut head = String::new();
        head.push_str(PROTOCOL);
        head.push(' ');
        head.push_str(self.header.method.as_str());
        head.push(' ');
        head.push_str(&self.header.resource.name);
        head.push(' ');
        let size_at = head.len();
        head.push('0'); // placeholder do message_size
        head.push('\n');

        for (key, value) in self.metadata.iter() {
            head.push_str(key);
            head.push_str(": ");
            head.push_str(value);
            head.push('\n');
        }
        head.push('\n');

        // message_size = tamanho TOTAL; os dígitos do tamanho influenciam o
        // próprio tamanho, então itera até os dígitos estabilizarem
        let base = head.len() - 1; // head sem o placeholder
        let mut digits = 1;
        let size = loop {
            let total = base + digits + payload_bytes.len();
            let d = total.to_string().len();
            if d == digits {
                break total;
            }
            digits = d;
        };

        head.replace_range(size_at..size_at + 1, &size.to_string());

        let mut message = head.into_bytes();
        message.extend_from_slice(payload_bytes);
        Ok(message)
    }
}