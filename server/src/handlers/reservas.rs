//! Itinerários e reservas: busca encadeando caronas e reserva atômica.

use ligerin_protocol::{Metadata, Method, Request, Resource, Response, ResponseHeader, Status, PROTOCOL};
use serde_json::json;

use crate::carona::{self, Erro, MAX_TRECHOS_VIAGEM, TrechoViagem};
use crate::lpc::{error_response, json_payload};
use crate::store::{data_valida, dono_atual, hoje_iso, perfil_atual, Store};


/// GET .itinerarios com payload {origem, destino, data}: combina até 3
/// caronas (trechos) entre origem e destino no dia.
pub(crate) fn itinerarios(request: &Request, store: &Store) -> Response {
    let recurso = &request.header.resource.name;
    if request.header.method != Method::GET {
        return error_response(
            Status::BadRequest,
            recurso,
            "use GET .itinerarios com payload {origem, destino, data}",
        );
    }
    let payload = request.payload.as_ref().and_then(|p| p.as_str());
    let Some(payload) = payload else {
        return error_response(
            Status::BadRequest,
            recurso,
            "GET .itinerarios precisa de payload JSON {origem, destino, data}",
        );
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return error_response(Status::BadRequest, recurso, "payload não é JSON válido");
    };
    let (Some(origem), Some(destino), Some(data)) = (
        value.get("origem").and_then(|v| v.as_str()),
        value.get("destino").and_then(|v| v.as_str()),
        value.get("data").and_then(|v| v.as_str()),
    ) else {
        return error_response(
            Status::BadRequest,
            recurso,
            "payload precisa de 'origem', 'destino' e 'data' (AAAA-MM-DD)",
        );
    };
    if !data_valida(data) {
        return error_response(Status::BadRequest, recurso, "data inválida (use AAAA-MM-DD)");
    }
    if data < hoje_iso().as_str() {
        return error_response(Status::BadRequest, recurso, "data no passado (use a partir de hoje)");
    }

    let lock = store.caronas.lock().expect("lock");
    let itinerarios = lock.itinerarios(origem, destino, data, MAX_TRECHOS_VIAGEM);
    let partidas = lock.partidas();
    Response::new(
        ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
        Metadata::new(),
        Some(json_payload(json!({
            "itinerarios": itinerarios.iter().map(|it| json!({
                "total_centavos": it.total_centavos,
                "trechos": it.trechos.iter().map(|p| carona::etapa_itinerario_json(p, &partidas)).collect::<Vec<_>>(),
            })).collect::<Vec<_>>()
        }))),
    )
}

/// .reservas: POST reserva (atômica, 409 se assento esgotado), GET lista,
/// POST .reservas.<id>.cancelar cancela e libera os assentos.
pub(crate) fn reservas(request: &Request, store: &Store) -> Response {
    let recurso = &request.header.resource.name;
    let id = request.header.resource.id();
    match (request.header.method, id) {
        (Method::POST, None) => criar_reserva(request, store),
        (Method::GET, None) => {
            let lock = store.caronas.lock().expect("lock");
            let partidas = lock.partidas();
            let mut lista = lock.lista_reservas();
            lista.sort_by_key(|r| r.id);
            let reservas: Vec<serde_json::Value> = lista
                .iter()
                .map(|r| {
                    carona::reserva_json(r, &partidas, store.nome_por_id(r.dono).as_deref())
                })
                .collect();
            drop(lock);
            Response::new(
                ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
                Metadata::new(),
                Some(json_payload(json!({ "reservas": reservas }))),
            )
        }
        (Method::POST, Some(id_s)) => {
            let Some(base) = id_s.strip_suffix(".cancelar") else {
                return error_response(
                    Status::BadRequest,
                    recurso,
                    "operação não suportada (use POST .reservas.<id>.cancelar)",
                );
            };
            let Ok(id) = base.parse::<u32>() else {
                return error_response(Status::BadRequest, recurso, "id da reserva deve ser número");
            };
            match store.caronas.lock().expect("lock").cancelar_reserva(id) {
                true => Response::new(
                    ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
                    Metadata::new(),
                    Some(json_payload(json!({ "cancelado": id }))),
                ),
                false => error_response(Status::NotFound, recurso, "reserva não encontrada"),
            }
        }
        (Method::GET, Some(_)) => error_response(
            Status::BadRequest,
            recurso,
            "use GET .reservas (lista) ou POST .reservas.<id>.cancelar",
        ),
    }
}

/// POST .reservas: valida todas as trechos da viagem e commita de uma vez
pub(crate) fn criar_reserva(request: &Request, store: &Store) -> Response {
    let recurso = &request.header.resource.name;
    let Some(dono) = dono_atual(request, store) else {
        return error_response(Status::Unauthorized, recurso, "sessão inválida");
    };
    if perfil_atual(request, store).as_deref() != Some("passageiro") {
        return error_response(
            Status::Forbidden,
            recurso,
            "apenas passageiros podem reservar assentos",
        );
    }
    let payload = request.payload.as_ref().and_then(|p| p.as_str());
    let Some(payload) = payload else {
        return error_response(Status::BadRequest, recurso, "POST .reservas precisa de payload JSON");
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return error_response(Status::BadRequest, recurso, "payload não é JSON válido");
    };
    let Some(trechos_json) = value.get("trechos").and_then(|p| p.as_array()) else {
        return error_response(Status::BadRequest, recurso, "payload precisa de 'trechos' (lista)");
    };
    if trechos_json.is_empty() {
        return error_response(Status::BadRequest, recurso, "reserva precisa de pelo menos um trecho");
    }
    if trechos_json.len() > MAX_TRECHOS_VIAGEM {
        return error_response(Status::BadRequest, recurso, format!("máximo de {MAX_TRECHOS_VIAGEM} trechos por reserva"));
    }

    let mut trechos: Vec<TrechoViagem> = Vec::with_capacity(trechos_json.len());
    for (i, p) in trechos_json.iter().enumerate() {
        let (Some(carona_id), Some(embarque), Some(desembarque)) = (
            p.get("carona_id").and_then(|c| c.as_u64()),
            p.get("embarque").and_then(|e| e.as_str()),
            p.get("desembarque").and_then(|d| d.as_str()),
        ) else {
            return error_response(
                Status::BadRequest,
                recurso,
                format!("trecho {i}: precisa de 'carona_id', 'embarque' e 'desembarque'"),
            );
        };
        let Ok(carona_id) = u32::try_from(carona_id) else {
            return error_response(Status::BadRequest, recurso, format!("trecho {i}: carona_id fora do intervalo"));
        };
        trechos.push(TrechoViagem {
            carona_id,
            embarque: embarque.to_string(),
            desembarque: desembarque.to_string(),
            disponiveis: 0, // só a busca de itinerários calcula lotação
        });
    }

    let mut lock = store.caronas.lock().expect("lock");
    match lock.reservar(dono, trechos) {
        Ok(reserva) => {
            let partidas = lock.partidas();
            let valor = carona::reserva_json(&reserva, &partidas, store.nome_por_id(reserva.dono).as_deref());
            drop(lock);
            Response::new(
                ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Created),
                Metadata::new(),
                Some(json_payload(json!({ "reserva": valor }))),
            )
        }
        Err(Erro::SemTrechos | Erro::TrechosDemais) => {
            error_response(Status::BadRequest, recurso, "trechos inválidas")
        }
        Err(Erro::CaronaInexistente) => error_response(Status::NotFound, recurso, "carona não encontrada"),
        Err(Erro::TrechoNaoAtendido) => {
            error_response(Status::NotFound, recurso, "embarque/desembarque não são paradas da carona")
        }
        Err(Erro::AssentoEsgotado) => {
            error_response(Status::Conflict, recurso, "assentos esgotados em um dos trechos")
        }
    }
}
