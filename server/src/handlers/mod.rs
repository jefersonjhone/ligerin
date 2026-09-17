//! Operações de cada recurso do LPC. Cada handler recebe o request já
//! parseado e o `Store` compartilhado, valida o payload campo a campo e
//! devolve a resposta (sucesso ou `error_response` com status e motivo).

mod auth;
mod caronas;
mod reservas;
mod rotas;

pub(crate) use auth::{login, logout, users};
pub(crate) use caronas::caronas;
pub(crate) use reservas::{itinerarios, reservas};
pub(crate) use rotas::{grafo, municipios, municipios_shapes, rota};
