//! Testes de integração do wire format LPC.
//!
//! Casos: pacotes válidos (GET/POST/response), GETs inválidos, tamanho
//! divergente e cabeçalho/metadata quebrado.

use ligerin_protocol::{
    Metadata, Method, ParseError, Payload, Request, RequestHeader, Resource, Response,
    ResponseHeader, Status,
};

/// Monta uma mensagem LPC com `message_size` calculado:
/// `LPC <rest> <size>\n<metadata>\n<payload>`
fn message(start_line_rest: &str, metadata: &str, payload: &str) -> String {
    let mut digits = 1;
    let size = loop {
        let total = 4 + start_line_rest.len() + 1 + digits + 1 + metadata.len() + 1 + payload.len();
        let d = total.to_string().len();
        if d == digits {
            break total;
        }
        digits = d;
    };
    format!("LPC {start_line_rest} {size}\n{metadata}\n{payload}")
}

// ─── pacotes válidos ───────────────────────────────────────────────────────────

#[test]
fn request_get_sem_payload() {
    let msg = message("GET .viagens", "", "");
    let req = Request::parse(&msg).expect("GET sem payload deve parsear");
    assert_eq!(req.header.method, Method::GET);
    assert_eq!(req.header.resource, Resource::new(".viagens"));
    assert_eq!(req.header.message_size, msg.len() as u32);
    assert!(req.metadata.iter().next().is_none());
    assert!(req.payload.is_none());
}

#[test]
fn request_get_com_metadata() {
    let msg = message("GET .users", "content-type: application/json", "");
    let req = Request::parse(&msg).expect("GET com metadata deve parsear");
    assert_eq!(req.metadata.get("content-type"), Some("application/json"));
    assert!(req.payload.is_none());
}

#[test]
fn request_post_com_payload_json() {
    let payload = r#"{"name":"Maria"}"#;
    let msg = message("POST .users", "", payload);
    let req = Request::parse(&msg).expect("POST com payload deve parsear");
    assert_eq!(req.header.method, Method::POST);
    assert_eq!(req.payload.expect("payload presente").as_str(), Some(payload));
}

#[test]
fn response_ok_com_payload() {
    let payload = r#"{"viagens":["FSA->SSA","SSA->FSA"]}"#;
    let msg = message(".viagens 200", "", payload);
    let res = Response::parse(&msg).expect("response OK deve parsear");
    assert_eq!(res.header.status, Status::Ok);
    assert_eq!(res.header.resource, Resource::new(".viagens"));
    assert_eq!(res.payload.expect("payload presente").as_str(), Some(payload));
}

#[test]
fn response_404_sem_payload() {
    let msg = message(".xyz 404", "", "");
    let res = Response::parse(&msg).expect("404 sem payload deve parsear");
    assert_eq!(res.header.status, Status::NotFound);
    assert!(res.payload.is_none());
}

// ─── GETs inválidos ────────────────────────────────────────────────────────────

#[test]
fn resource_sem_prefixo_ponto() {
    let msg = message("GET viagens", "", "");
    let err = Request::parse(&msg).expect_err("recurso sem '.' deve falhar");
    assert!(matches!(err, ParseError::InvalidHeader { field: "resource", .. }));
}

#[test]
fn resource_com_barra() {
    let msg = message("GET .a/b", "", "");
    let err = Request::parse(&msg).expect_err("resource com '/' deve falhar");
    assert!(matches!(err, ParseError::InvalidHeader { field: "resource", .. }));
}

#[test]
fn metodo_desconhecido() {
    let msg = message("FETCH .viagens", "", "");
    let err = Request::parse(&msg).expect_err("método desconhecido deve falhar");
    assert!(matches!(err, ParseError::InvalidHeader { field: "method", .. }));
}

#[test]
fn protocolo_desconhecido() {
    let err = Request::parse("XYZ GET .viagens 21\n\n")
        .expect_err("protocolo desconhecido deve falhar");
    assert!(matches!(err, ParseError::InvalidHeader { field: "start-line", .. }));
}

#[test]
fn start_line_incompleta() {
    let err = Request::parse("LPC GET .viagens\n\n").expect_err("3 tokens deve falhar");
    assert!(matches!(err, ParseError::InvalidHeader { field: "start-line", .. }));
}

#[test]
fn mensagem_vazia() {
    let err = Request::parse("").expect_err("mensagem vazia deve falhar");
    assert!(matches!(err, ParseError::InvalidHeader { field: "start-line", .. }));
}

// ─── tamanho divergente ────────────────────────────────────────────────────────

#[test]
fn tamanho_menor_que_o_real() {
    let err = Request::parse("LPC GET .viagens 10\n\n")
        .expect_err("message_size menor que o real deve falhar");
    assert!(matches!(err, ParseError::InvalidPayload { field: "message_size", .. }));
}

#[test]
fn tamanho_maior_que_o_real() {
    let err = Request::parse("LPC GET .viagens 30\n\n")
        .expect_err("message_size maior que o real deve falhar");
    assert!(matches!(err, ParseError::InvalidPayload { field: "message_size", .. }));
}

#[test]
fn tamanho_nao_numerico() {
    let err = Request::parse("LPC GET .viagens abc\n\n")
        .expect_err("message_size não numérico deve falhar");
    assert!(matches!(err, ParseError::InvalidHeader { field: "message_size", .. }));
}

// ─── cabeçalho/metadata quebrado ───────────────────────────────────────────────

#[test]
fn metadata_sem_dois_pontos() {
    let msg = message("GET .viagens", "chave sem valor", "");
    let err = Request::parse(&msg).expect_err("linha sem ':' deve falhar");
    assert!(matches!(err, ParseError::InvalidMetadata { field: "linha", .. }));
}

#[test]
fn metadata_chave_vazia() {
    let msg = message("GET .viagens", ": valor", "");
    let err = Request::parse(&msg).expect_err("chave vazia deve falhar");
    assert!(matches!(err, ParseError::InvalidMetadata { field: "chave", .. }));
}

#[test]
fn post_sem_payload() {
    let msg = message("POST .users", "", "");
    let err = Request::parse(&msg).expect_err("POST sem payload deve falhar");
    assert!(matches!(err, ParseError::InvalidPayload { field: "payload", .. }));
}

#[test]
fn response_status_desconhecido() {
    let err = Response::parse("LPC .viagens 999 22\n\n")
        .expect_err("status desconhecido deve falhar");
    assert!(matches!(err, ParseError::InvalidHeader { field: "status", .. }));
}

// ─── round-trip (encode → parse) ───────────────────────────────────────────────

#[test]
fn round_trip_request() {
    let req = Request::new(
        RequestHeader::new("LPC".into(), Resource::new(".users"), 0, Method::POST),
        Metadata::new(),
        Some(Payload::from_str(r#"{"name":"Maria"}"#)),
    );
    let bytes = req.encode().expect("encode deve funcionar");
    let parsed =
        Request::parse(std::str::from_utf8(&bytes).unwrap()).expect("parse do round-trip");

    // message_size é derivado no encode — não participa da comparação
    assert_eq!(parsed.header.method, req.header.method);
    assert_eq!(parsed.header.resource, req.header.resource);
    assert_eq!(parsed.metadata, req.metadata);
    assert_eq!(parsed.payload, req.payload);
}

#[test]
fn round_trip_response() {
    let res = Response::new(
        ResponseHeader::new("LPC".into(), Resource::new(".viagens"), 0, Status::Ok),
        Metadata::new(),
        Some(Payload::from_str(r#"{"viagens":["FSA->SSA","SSA->FSA"]}"#)),
    );
    let bytes = res.encode();
    let parsed = Response::parse(std::str::from_utf8(&bytes).unwrap()).expect("parse do round-trip");

    assert_eq!(parsed.header.status, res.header.status);
    assert_eq!(parsed.header.resource, res.header.resource);
    assert_eq!(parsed.metadata, res.metadata);
    assert_eq!(parsed.payload, res.payload);
}

#[test]
fn status_conflict_409_parseia_e_volta() {
    let payload = r#"{"error":"assento esgotado"}"#;
    let msg = message(".reservas 409", "", payload);
    let res = Response::parse(&msg).expect("409 deve parsear");
    assert_eq!(res.header.status, Status::Conflict);
    assert_eq!(res.header.resource, Resource::new(".reservas"));
    assert_eq!(res.payload.expect("payload presente").as_str(), Some(payload));

    // round-trip pelo encode
    let res = Response::new(
        ResponseHeader::new("LPC".into(), Resource::new(".reservas"), 0, Status::Conflict),
        Metadata::new(),
        Some(Payload::from_str(payload)),
    );
    let bytes = res.encode();
    let parsed = Response::parse(std::str::from_utf8(&bytes).expect("utf8")).expect("parse");
    assert_eq!(parsed.header.status, Status::Conflict);
    assert_eq!(parsed.payload, res.payload);
}