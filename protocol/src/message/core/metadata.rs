use std::collections::HashMap;

use crate::core::error::ParseError;

/// Chave reservada de metadata: correlação request/response.
///
/// O client gera um valor único por request; o server ecoa o mesmo valor
/// na resposta. Casar resposta com request quando há vários clientes ou
/// requests em andamento.
pub const MESSAGE_ID: &str = "message-id";

/// Chave reservada de metadata: token de autenticação.
///
/// O client envia após login (`POST .login` → `{"token": ...}`); o server
/// valida antes de rotear recursos protegidos.
pub const AUTHORIZATION: &str = "authorization";

/// Pares chave/valor que acompanham a mensagem (análogo aos headers do HTTP).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metadata {
    fields: HashMap<String, String>,
}

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.fields.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.fields.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Parseia o bloco de metadados: linhas `chave: valor` até a primeira
    /// linha vazia (que termina o bloco).
    pub fn parse<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<Self, ParseError> {
        let mut fields = HashMap::new();
        for line in lines {
            let line = line.trim_end_matches('\n');
            if line.is_empty() {
                break;
            }
            let Some((key, value)) = line.split_once(':') else {
                return Err(ParseError::invalid_metadata("linha", "chave: valor", line));
            };
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() {
                return Err(ParseError::invalid_metadata("chave", "nome não vazio", line));
            }
            fields.insert(key.to_string(), value.to_string());
        }
        Ok(Self { fields })
    }
}