use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use ligerin_protocol::{
    AUTHORIZATION, MESSAGE_ID, Metadata, Method, Payload, Request, RequestHeader, Resource,
    Response, ResponseHeader, PROTOCOL,
};

pub struct LPCClient {
    pub ip: String,
    pub port: String,
}

#[allow(dead_code)] // get/post são API pronta, ainda não chamadas pelo command
impl LPCClient {
    pub fn get(&self, resource: &str, token: Option<&str>) -> Result<Response, String> {
        self.request(Method::GET, resource, None, token)
    }

    pub fn post(
        &self,
        resource: &str,
        payload: &str,
        token: Option<&str>,
    ) -> Result<Response, String> {
        self.request(Method::POST, resource, Some(payload), token)
    }

    /// Autentica e devolve o token de sessão.
    pub fn login(&self, nome: &str, senha: &str) -> Result<String, String> {
        let payload = serde_json::json!({ "nome": nome, "senha": senha }).to_string();
        let response = self.request(Method::POST, ".login", Some(&payload), None)?;
        let payload = response
            .payload
            .as_ref()
            .and_then(|payload| payload.as_str())
            .ok_or("resposta de login sem payload")?;
        let value: serde_json::Value =
            serde_json::from_str(payload).map_err(|_| "payload de login inválido")?;
        value
            .get("token")
            .and_then(|token| token.as_str())
            .map(String::from)
            .ok_or_else(|| "resposta sem token".to_string())
    }

    /// Encerra a sessão do token.
    pub fn logout(&self, token: &str) -> Result<Response, String> {
        // POST exige payload por regra do protocolo — `{}` é o corpo vazio
        self.request(Method::POST, ".logout", Some("{}"), Some(token))
    }

    /// Ponto único: monta o request pelo protocolo, envia, enquadra a
    /// resposta pelo `message_size` e decodifica.
    pub fn request(
        &self,
        method: Method,
        resource: &str,
        payload: Option<&str>,
        token: Option<&str>,
    ) -> Result<Response, String> {
        // correlação: um message-id novo por request
        let message_id = new_message_id();
        let mut metadata = Metadata::new();
        metadata.insert(MESSAGE_ID, &message_id);
        if let Some(token) = token {
            metadata.insert(AUTHORIZATION, token);
        }

        let request = Request::new(
            RequestHeader::new(
                PROTOCOL.to_string(),
                Resource::parse(resource).map_err(|err| err.to_string())?,
                0, // message_size é derivado no encode
                method,
            ),
            metadata,
            payload.map(Payload::from_str),
        );
        let bytes = request.encode().map_err(|err| err.to_string())?;

        let mut stream = TcpStream::connect(format!("{}:{}", self.ip, self.port))
            .map_err(|err| format!("erro ao conectar: {err}"))?;
        stream.write_all(&bytes).map_err(|err| format!("erro ao enviar: {err}"))?;

        // framing: start-line → message_size → resto exato
        let mut reader = BufReader::new(stream);
        let mut start_line = String::new();
        reader
            .read_line(&mut start_line)
            .map_err(|err| format!("erro ao ler resposta: {err}"))?;

        let header = ResponseHeader::parse(start_line.trim_end_matches('\n'))
            .map_err(|err| err.to_string())?;
        let total = header.message_size as usize;
        if total < start_line.len() {
            return Err(format!("message_size ({total}) menor que a start-line"));
        }

        let mut rest = vec![0u8; total - start_line.len()];
        reader
            .read_exact(&mut rest)
            .map_err(|err| format!("frame incompleto: {err}"))?;

        let mut message = start_line;
        message.push_str(
            &String::from_utf8(rest).map_err(|_| "resposta não é texto UTF-8".to_string())?,
        );

        let response = Response::parse(&message).map_err(|err| err.to_string())?;

        // o server deve ter ecoado o mesmo message-id
        match response.metadata.get(MESSAGE_ID) {
            Some(id) if id == message_id => Ok(response),
            Some(_) => Err("resposta com message-id divergente".to_string()),
            None => Err("resposta sem message-id".to_string()),
        }
    }
}

fn new_message_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{}", std::process::id(), nanos)
}