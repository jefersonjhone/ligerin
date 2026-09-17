//! Rotas e mapa: cálculo de rota no grafo, consultas de municípios e shapes.

use ligerin_protocol::{Metadata, Method, Request, Resource, Response, ResponseHeader, Status, PROTOCOL};
use serde_json::json;

use crate::lpc::{error_response, json_payload};
use crate::store::{arredondar, arredondar_coord, Store};


/// GET .rota com payload {origem, destino, via?}: devolve a rota mais curta
/// (menor km) e os municípios atravessados — o rascunho da carona do motorista.
pub(crate) fn rota(request: &Request, store: &Store) -> Response {
    let recurso = request.header.resource.name.as_str();
    if request.header.method != Method::GET {
        return error_response(
            Status::BadRequest,
            recurso,
            "use GET .rota com payload {origem, destino}",
        );
    }
    let payload = request.payload.as_ref().and_then(|p| p.as_str());
    let Some(payload) = payload else {
        return error_response(
            Status::BadRequest,
            recurso,
            "GET .rota precisa de payload JSON {origem, destino}",
        );
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return error_response(Status::BadRequest, recurso, "payload não é JSON válido");
    };
    let (Some(origem), Some(destino)) = (
        value.get("origem").and_then(|o| o.as_str()),
        value.get("destino").and_then(|d| d.as_str()),
    ) else {
        return error_response(
            Status::BadRequest,
            recurso,
            "payload precisa de 'origem' e 'destino'",
        );
    };
    let via: Vec<String> = value
        .get("via")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let rota = if via.is_empty() {
        store.grafo.rota(origem, destino)
    } else {
        store.grafo.rota_com_via(origem, destino, &via)
    };
    let Some(rota) = rota else {
        return error_response(
            Status::NotFound,
            recurso,
            "sem rota entre origem e destino (ou município desconhecido)",
        );
    };

    let trechos: Vec<serde_json::Value> = rota
        .trechos
        .iter()
        .map(|t| {
            json!({ "id": t.id, "mun": t.mun, "rodovia": t.rodovia, "extensao_km": t.extensao_km })
        })
        .collect();
    let linha: Vec<[f64; 2]> =
        rota.linha.iter().copied().map(arredondar_coord).collect();
    let marcadores: Vec<serde_json::Value> = rota
        .marcadores
        .iter()
        .map(|m| json!({ "nome": m.nome, "coord": arredondar_coord(m.coord) }))
        .collect();
    Response::new(
        ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
        Metadata::new(),
        Some(json_payload(json!({
            "origem": rota.origem,
            "destino": rota.destino,
            "distancia_km": arredondar(rota.distancia_km),
            "municipios": rota.municipios,
            "trechos": trechos,
            "linha": linha,
            "marcadores": marcadores,
        }))),
    )
}

/// .grafo.cidades (nomes) e .grafo.<id> (segmento); a raiz sem id é 400.
pub(crate) fn grafo(request: &Request, store: &Store) -> Response {
    let recurso = request.header.resource.name.as_str();
    match request.header.resource.id() {
        None => error_response(
            Status::BadRequest,
            recurso,
            "use .grafo.cidades ou .grafo.<id-do-segmento>",
        ),
        Some("cidades") => Response::new(
            ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
            Metadata::new(),
            Some(json_payload(json!({ "cidades": store.grafo.cidades() }))),
        ),
        Some(id) => match store.grafo.no(id) {
            Some(no) => Response::new(
                ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
                Metadata::new(),
                Some(json_payload(json!({
                    "id": id,
                    "mun": no.mun,
                    "rodovia": no.rodovia,
                    "extensao_km": no.extensao_km,
                    "conec": no.conec,
                }))),
            ),
            None => error_response(
                Status::NotFound,
                recurso,
                format!("segmento não encontrado: {id}"),
            ),
        },
    }
}

/// .municipios.shapes: polígonos por município para o mapa offline. GET com
/// payload opcional {municipios:[...]} filtra; sem payload devolve todos.
pub(crate) fn municipios_shapes(request: &Request, store: &Store) -> Response {
    let recurso = request.header.resource.name.as_str();
    if request.header.method != Method::GET {
        return error_response(
            Status::BadRequest,
            recurso,
            "use GET .municipios.shapes (payload opcional {municipios:[...]})",
        );
    }
    let pedidos: Option<Vec<String>> = match request
        .payload
        .as_ref()
        .and_then(|p| p.as_str())
        .and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok())
    {
        Some(v) => v
            .get("municipios")
            .and_then(|m| m.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()),
        None => None,
    };
    let shapes: serde_json::Map<String, serde_json::Value> = match pedidos {
        // só os pedidos que existem (nomes desconhecidos são ignorados)
        Some(nomes) => nomes
            .iter()
            .filter_map(|nome| store.shapes.get(nome).map(|s| (nome.clone(), s.clone())))
            .collect(),
        None => store.shapes.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
    };
    Response::new(
        ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
        Metadata::new(),
        Some(json_payload(json!({ "shapes": shapes }))),
    )
}

/// .municipios: nomes dos municípios (o handler de `.municipios.shapes` é
/// registrado à parte; qualquer outro id é 404).
pub(crate) fn municipios(request: &Request, store: &Store) -> Response {
    let recurso = request.header.resource.name.as_str();
    match request.header.resource.id() {
        None => Response::new(
            ResponseHeader::new(PROTOCOL.to_string(), Resource::new(recurso), 0, Status::Ok),
            Metadata::new(),
            Some(json_payload(json!({ "municipios": store.grafo.cidades() }))),
        ),
        Some(_) => error_response(
            Status::NotFound,
            recurso,
            "recurso desconhecido em .municipios (use .municipios ou .municipios.shapes)",
        ),
    }
}
