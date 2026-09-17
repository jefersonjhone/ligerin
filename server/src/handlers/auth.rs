//! Sessão e usuários: registro, autenticação e encerramento de sessão.

use ligerin_protocol::{AUTHORIZATION, Metadata, Method, Request, Resource, Response, ResponseHeader, Status, PROTOCOL};
use serde_json::json;
use std::sync::atomic::Ordering;

use crate::lpc::{error_response, json_payload};
use crate::store::{Usuario, Store};


pub(crate) fn users(request: &Request, store: &Store) -> Response {
    match request.header.method {
        Method::GET => {
            let usuarios = store.usuarios.lock().expect("lock");
            let nomes: Vec<&str> = usuarios.keys().map(String::as_str).collect();
            Response::new(
                ResponseHeader::new(PROTOCOL.to_string(), Resource::new(".users"), 0, Status::Ok),
                Metadata::new(),
                Some(json_payload(json!({ "users": nomes }))),
            )
        }
        Method::POST => {
            let payload = request.payload.as_ref().and_then(|payload| payload.as_str());
            let Some(payload) = payload else {
                return error_response(Status::BadRequest, ".users", "POST sem payload JSON");
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
                return error_response(Status::BadRequest, ".users", "payload não é JSON válido");
            };
            let (Some(nome), Some(senha)) = (
                value.get("nome").and_then(|nome| nome.as_str()),
                value.get("senha").and_then(|senha| senha.as_str()),
            ) else {
                return error_response(
                    Status::BadRequest,
                    ".users",
                    "payload precisa de 'nome' e 'senha'",
                );
            };
            if nome.is_empty() || senha.is_empty() {
                return error_response(
                    Status::BadRequest,
                    ".users",
                    "'nome' e 'senha' não podem ser vazios",
                );
            }
            let profiletype = value
                .get("profiletype")
                .and_then(|profiletype| profiletype.as_str())
                .unwrap_or("passageiro");
            if !["motorista", "passageiro"].contains(&profiletype) {
                return error_response(
                    Status::BadRequest,
                    ".users",
                    "profiletype deve ser 'motorista' ou 'passageiro'",
                );
            }
            let mut usuarios = store.usuarios.lock().expect("lock");
            if usuarios.contains_key(nome) {
                return error_response(
                    Status::Conflict,
                    ".users",
                    format!("usuário '{nome}' já existe"),
                );
            }
            let id = u32::try_from(store.prox_user.fetch_add(1, Ordering::Relaxed))
                .expect("id de usuário cabe em u32");
            usuarios.insert(
                nome.to_string(),
                Usuario {
                    id,
                    senha: senha.to_string(),
                    profiletype: profiletype.to_string(),
                },
            );
            store.nomes_por_id.lock().expect("lock").insert(id, nome.to_string());
            drop(usuarios);
            Response::new(
                ResponseHeader::new(
                    PROTOCOL.to_string(),
                    Resource::new(".users"),
                    0,
                    Status::Created,
                ),
                Metadata::new(),
                Some(json_payload(json!({
                    "user": nome,
                    "id": id,
                    "profiletype": profiletype,
                }))),
            )
        }
    }
}

pub(crate) fn login(request: &Request, store: &Store) -> Response {
    let payload = request.payload.as_ref().and_then(|payload| payload.as_str());
    let Some(payload) = payload else {
        return error_response(Status::BadRequest, ".login", "POST sem payload JSON");
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return error_response(Status::BadRequest, ".login", "payload não é JSON válido");
    };
    let (Some(nome), Some(senha)) = (
        value.get("nome").and_then(|nome| nome.as_str()),
        value.get("senha").and_then(|senha| senha.as_str()),
    ) else {
        return error_response(
            Status::BadRequest,
            ".login",
            "payload precisa de 'nome' e 'senha'",
        );
    };

    match store.autenticar(nome, senha) {
        Some((token, profiletype, id)) => Response::new(
            ResponseHeader::new(PROTOCOL.to_string(), Resource::new(".login"), 0, Status::Ok),
            Metadata::new(),
            Some(json_payload(json!({
                "token": token,
                "profiletype": profiletype,
                "id": id,
            }))),
        ),
        None => error_response(Status::Unauthorized, ".login", "credenciais inválidas"),
    }
}

pub(crate) fn logout(request: &Request, store: &Store) -> Response {
    if let Some(token) = request.metadata.get(AUTHORIZATION) {
        store.revogar(token);
    }
    Response::new(
        ResponseHeader::new(
            PROTOCOL.to_string(),
            Resource::new(".logout"),
            0,
            Status::Ok,
        ),
        Metadata::new(),
        Some(json_payload(json!({ "logout": "ok" }))),
    )
}
