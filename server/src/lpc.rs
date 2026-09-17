//! Roteador do protocolo LPC: registro de recursos e despacho de requests.
//! O enquadramento (start-line/metadados/payload) vive no crate `protocol`;
//! aqui cada recurso registrado é roteado para o handler correspondente.

use std::collections::HashMap;

use ligerin_protocol::{
    AUTHORIZATION, MESSAGE_ID, Metadata, Method, Payload, Request, Resource, Response,
    ResponseHeader, Status, PROTOCOL,
};
use serde_json::json;

use crate::handlers::{
    caronas, grafo, itinerarios, login, logout, municipios,
    municipios_shapes, reservas, rota, users,
};
use crate::store::Store;

/// Manipulador de um recurso: recebe o request parseado e devolve a resposta.
type Handler = fn(&Request, &Store) -> Response;

pub(crate) struct LPC {
    resources: HashMap<String, Handler>,
    pub(crate) store: Store,
}

impl LPC {
    /// Server com dados reais (grafo/malha carregados da pasta data/).
    pub(crate) fn new() -> Self {
        Self::com_registros(Store::new())
    }

    /// Registra todos os recursos do domínio.
    pub(crate) fn com_registros(store: Store) -> Self {
        let mut lpc = Self { resources: HashMap::new(), store };
        lpc.register(".login", login);
        lpc.register(".logout", logout);
        lpc.register(".users", users);
        lpc.register(".rota", rota);
        lpc.register(".grafo", grafo);
        lpc.register(".municipios", municipios);
        lpc.register(".municipios.shapes", municipios_shapes);
        lpc.register(".caronas", caronas);
        lpc.register(".itinerarios", itinerarios);
        lpc.register(".reservas", reservas);
        lpc
    }

    fn register(&mut self, resource: &str, handler: Handler) {
        self.resources.insert(resource.to_string(), handler);
    }

    pub(crate) fn handle(&self, request: &Request) -> Response {
        // públicos: .login e registro (POST .users); o resto exige token
        let publico = request.header.resource.name == ".login"
            || (request.header.resource.name == ".users" && request.header.method == Method::POST);

        if !publico {
            match request.metadata.get(AUTHORIZATION) {
                Some(token) if self.store.token_valido(token) => {}
                Some(_) => {
                    return error_response(
                        Status::Unauthorized,
                        &request.header.resource.name,
                        "token inválido",
                    )
                }
                None => {
                    return error_response(
                        Status::Unauthorized,
                        &request.header.resource.name,
                        "token ausente",
                    )
                }
            }
        }

        let resource = &request.header.resource.name;
        let handler = self
            .resources
            .get(resource)
            .or_else(|| self.resources.get(request.header.resource.raiz()));
        let mut response = match handler {
            Some(handler) => handler(request, &self.store),
            // 404 ecoa o resource pedido
            None => error_response(
                Status::NotFound,
                resource,
                format!("recurso não encontrado: {resource}"),
            ),
        };

        // correlação: ecoa o message-id do request na resposta
        if let Some(id) = request.metadata.get(MESSAGE_ID) {
            response.metadata.insert(MESSAGE_ID, id);
        }

        response
    }
}

/// Monta um payload JSON a partir de um valor serde_json.
pub(crate) fn json_payload(value: serde_json::Value) -> Payload {
    Payload::from_str(value.to_string())
}

/// Resposta de erro com corpo JSON.
pub(crate) fn error_response(status: Status, resource: &str, message: impl Into<String>) -> Response {
    Response::new(
        ResponseHeader::new(PROTOCOL.to_string(), Resource::new(resource), 0, status),
        Metadata::new(),
        Some(json_payload(json!({ "error": message.into() }))),
    )
}
