use crate::core::error::ParseError;

/// Recurso alvo de uma mensagem — semelhante ao path do HTTP.
///
/// No LPC, recursos usam o prefixo `.` em vez das barras do HTTP:
/// `.viagens`, `.users`, `.cidades`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Resource {
    pub name: String,
}

impl Resource {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Regras: começa com `.` e não contém `/` (a convenção do `.` substitui
    /// os paths com barra do HTTP).
    pub fn parse(name: &str) -> Result<Self, ParseError> {
        if !name.starts_with('.') {
            return Err(ParseError::invalid_header(
                "resource",
                "recurso com prefixo '.' (ex.: .viagens)",
                name,
            ));
        }
        if name.contains('/') {
            return Err(ParseError::invalid_header(
                "resource",
                "recurso sem '/' (a convenção usa '.')",
                name,
            ));
        }
        Ok(Self::new(name))
    }

    /// Raiz de um recurso composto (`.carona.5` → `.carona`; `.login` → `.login`).
    pub fn raiz(&self) -> &str {
        self.name[1..]
            .find('.')
            .map_or(self.name.as_str(), |i| &self.name[..i + 1])
    }

    /// Id de um recurso composto (`.carona.5` → `Some("5")`; `.login` → `None`).
    pub fn id(&self) -> Option<&str> {
        self.name[1..].find('.').map(|i| &self.name[i + 2..])
    }
}