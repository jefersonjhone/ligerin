//! Caronas: publicação com validação de rota, listagem e cancelamento.

use std::collections::HashSet;

use ligerin_protocol::{Metadata, Method, Request, Resource, Response, ResponseHeader, Status, PROTOCOL};
use serde_json::json;

use crate::carona::{self, MAX_TRECHOS_CARONA};
use crate::lpc::{error_response, json_payload};
use crate::store::{dono_atual, hoje_iso, partida_valida, perfil_atual, Store};


/// .caronas: POST publica (payload com trechos encadeados e preço por trecho),
/// GET lista; .caronas.<id> consulta; POST .caronas.<id>.cancelar cancela.
pub(crate) fn caronas(request: &Request, store: &Store) -> Response {
    let recurso = &request.header.resource.name;
    let id = request.header.resource.id();
    match (request.header.method, id) {
        (Method::POST, None) => publicar_carona(request, store),
        (Method::GET, None) => {
            let lock = store.caronas.lock().expect("lock");
            let mut lista = lock.lista();
            lista.sort_by_key(|c| c.id);
            let caronas: Vec<serde_json::Value> = lista
                .iter()
                .map(|c| carona::carona_json(c, store.nome_por_id(c.dono).as_deref()))
                .collect();
            drop(lock);
            Response::new(
                ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
                Metadata::new(),
                Some(json_payload(json!({ "caronas": caronas }))),
            )
        }
        (Method::GET, Some(id_s)) => {
            let Ok(id) = id_s.parse::<u32>() else {
                return error_response(Status::BadRequest, recurso, "id da carona deve ser número");
            };
            match store.caronas.lock().expect("lock").obter(id) {
                Some(carona) => {
                    let valor = carona::carona_json(carona, store.nome_por_id(carona.dono).as_deref());
                    Response::new(
                        ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
                        Metadata::new(),
                        Some(json_payload(json!({ "carona": valor }))),
                    )
                }
                None => error_response(Status::NotFound, recurso, "carona não encontrada"),
            }
        }
        (Method::POST, Some(id_s)) => {
            let Some(base) = id_s.strip_suffix(".cancelar") else {
                return error_response(
                    Status::BadRequest,
                    recurso,
                    "operação não suportada (use POST .caronas.<id>.cancelar)",
                );
            };
            cancelar_carona(request, store, base)
        }
    }
}

/// POST .caronas: valida a cadeia de trechos e publica.
pub(crate) fn publicar_carona(request: &Request, store: &Store) -> Response {
    let recurso = &request.header.resource.name;
    let Some(dono) = dono_atual(request, store) else {
        return error_response(Status::Unauthorized, recurso, "sessão inválida");
    };
    if perfil_atual(request, store).as_deref() != Some("motorista") {
        return error_response(
            Status::Forbidden,
            recurso,
            "apenas motoristas podem publicar caronas",
        );
    }
    let payload = request.payload.as_ref().and_then(|p| p.as_str());
    let Some(payload) = payload else {
        return error_response(Status::BadRequest, recurso, "POST .caronas precisa de payload JSON");
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return error_response(Status::BadRequest, recurso, "payload não é JSON válido");
    };

    let Some(partida) = value.get("partida").and_then(|p| p.as_str()) else {
        return error_response(
            Status::BadRequest,
            recurso,
            "payload precisa de 'partida' (AAAA-MM-DDTHH:MM)",
        );
    };
    if !partida_valida(partida) {
        return error_response(Status::BadRequest, recurso, "partida inválida (use AAAA-MM-DDTHH:MM)");
    }
    let hoje = hoje_iso();
    if &partida[..10] < hoje.as_str() {
        return error_response(Status::BadRequest, recurso, "partida no passado (use a partir de hoje)");
    }
    let Some(trechos_json) = value.get("trechos").and_then(|t| t.as_array()) else {
        return error_response(Status::BadRequest, recurso, "payload precisa de 'trechos' (lista)");
    };
    if trechos_json.is_empty() {
        return error_response(Status::BadRequest, recurso, "carona precisa de pelo menos um trecho");
    }
    if trechos_json.len() > MAX_TRECHOS_CARONA {
        return error_response(Status::BadRequest, recurso, format!("máximo de {MAX_TRECHOS_CARONA} trechos"));
    }

    // parse + encadeamento + paradas únicas
    let mut brutos: Vec<(String, String, u32, u32)> = Vec::with_capacity(trechos_json.len());
    let mut vistos: HashSet<String> = HashSet::new();
    let mut ultimo_destino: Option<&str> = None;
    for (i, t) in trechos_json.iter().enumerate() {
        let (Some(origem), Some(destino)) = (
            t.get("origem").and_then(|o| o.as_str()),
            t.get("destino").and_then(|d| d.as_str()),
        ) else {
            return error_response(
                Status::BadRequest,
                recurso,
                format!("trecho {i}: precisa de 'origem' e 'destino'"),
            );
        };
        let Some(preco) = t.get("preco").and_then(|p| p.as_u64()) else {
            return error_response(
                Status::BadRequest,
                recurso,
                format!("trecho {i}: precisa de 'preco' (centavos, inteiro)"),
            );
        };
        let Ok(preco) = u32::try_from(preco) else {
            return error_response(Status::BadRequest, recurso, format!("trecho {i}: preco fora do intervalo"));
        };
        let Some(assentos) = t.get("assentos").and_then(|a| a.as_u64()) else {
            return error_response(
                Status::BadRequest,
                recurso,
                format!("trecho {i}: precisa de 'assentos' (inteiro)"),
            );
        };
        let Ok(assentos) = u32::try_from(assentos) else {
            return error_response(Status::BadRequest, recurso, format!("trecho {i}: assentos fora do intervalo"));
        };
        if assentos == 0 || assentos > 46 {
            return error_response(Status::BadRequest, recurso, format!("trecho {i}: assentos deve estar entre 1 e 1000"));
        }
        if origem.is_empty() || destino.is_empty() {
            return error_response(Status::BadRequest, recurso, format!("trecho {i}: origem/destino vazios"));
        }
        if let Some(ult) = ultimo_destino {
            if origem != ult {
                return error_response(
                    Status::BadRequest,
                    recurso,
                    format!("trechos não encadeiam: {ult} != {origem} (trecho {i})"),
                );
            }
        }
        if !vistos.insert(destino.to_string()) {
            return error_response(Status::BadRequest, recurso, "município repetido na rota (cada parada uma única vez)");
        }
        if i == 0 {
            vistos.insert(origem.to_string());
        }
        ultimo_destino = Some(destino);
        brutos.push((origem.to_string(), destino.to_string(), preco, assentos));
    }

    // conectividade no grafo + km de cada trecho
    let mut trechos: Vec<(String, String, u32, f64, u32)> = Vec::with_capacity(brutos.len());
    for (origem, destino, preco, assentos) in brutos {
        let Some(rota) = store.grafo.rota(&origem, &destino) else {
            return error_response(
                Status::BadRequest,
                recurso,
                format!("sem rota entre {origem} e {destino}"),
            );
        };
        trechos.push((origem, destino, preco, rota.distancia_km, assentos));
    }

    let mut lock = store.caronas.lock().expect("lock");
    lock.podar_passadas(&hoje);
    let carona = lock.publicar(dono, partida, trechos);
    Response::new(
        ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Created),
        Metadata::new(),
        Some(json_payload(json!({
            "carona": carona::carona_json(&carona, store.nome_por_id(carona.dono).as_deref())
        }))),
    )
}

/// POST .caronas.<id>.cancelar: só o dono; deleta as reservas da carona.
pub(crate) fn cancelar_carona(request: &Request, store: &Store, id_s: &str) -> Response {
    let recurso = &request.header.resource.name;
    let Ok(id) = id_s.parse::<u32>() else {
        return error_response(Status::BadRequest, recurso, "id da carona deve ser número");
    };
    let Some(dono) = dono_atual(request, store) else {
        return error_response(Status::Unauthorized, recurso, "sessão inválida");
    };
    let mut lock = store.caronas.lock().expect("lock");
    let Some(carona) = lock.obter(id) else {
        return error_response(Status::NotFound, recurso, "carona não encontrada");
    };
    if carona.dono != dono {
        return error_response(Status::Forbidden, recurso, "só o dono pode cancelar a carona");
    }
    lock.remover_carona(id);
    Response::new(
        ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
        Metadata::new(),
        Some(json_payload(json!({ "cancelado": id }))),
    )
}
