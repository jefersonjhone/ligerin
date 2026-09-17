//! Suíte de integração: sobe o servidor LPC numa porta efêmera e exercita o
//! protocolo de ponta a ponta (autenticação, rotas, caronas, itinerários,
//! reservas atômicas e corridas de assentos sob carga).

use crate::grafo::Grafo;
use crate::lpc::LPC;
use chrono::Utc;

use crate::store::{Store, Usuario};
use ligerin_protocol::{
    AUTHORIZATION, MESSAGE_ID, Metadata, Method, Payload, Request, RequestHeader, Resource,
    Response, ResponseHeader, Status,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Barrier, Mutex};
use std::sync::atomic::AtomicU64;
use std::time::Duration;



use crate::serve;

/// Sobe o server numa porta efêmera e devolve a porta.
fn start_server() -> u16 {
    serve_com(LPC::new())
}

/// Sobe um server construído à mão (grafo sintético nos testes).
fn serve_com(lpc: LPC) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || serve(listener, Arc::new(lpc)));
    port
}

/// Grafo sintético com 4 municípios (mesmo desenho de grafo::tests).
fn grafo_sintetico() -> Grafo {
    use crate::grafo::No;
    let nos = HashMap::from([
        ("salvador".to_string(), No { mun: vec!["Salvador".into()], rodovia: Some("BR-324".into()), extensao_km: Some(1.0), geometry: None, conec: vec!["santo_amaro".into(), "jequie".into()] }),
        ("santo_amaro".to_string(), No { mun: vec!["Santo Amaro".into()], rodovia: Some("BR-324".into()), extensao_km: Some(1.0), geometry: None, conec: vec!["salvador".into(), "feira".into()] }),
        ("jequie".to_string(), No { mun: vec!["Jequié".into()], rodovia: Some("BR-116".into()), extensao_km: Some(100.0), geometry: None, conec: vec!["salvador".into(), "feira".into()] }),
        ("feira".to_string(), No { mun: vec!["Feira de Santana".into()], rodovia: Some("BR-324".into()), extensao_km: Some(1.0), geometry: None, conec: vec!["santo_amaro".into(), "jequie".into()] }),
    ]);
    Grafo::novo(nos)
}

/// Store com grafo sintético (não depende dos arquivos de dados).
fn store_sintetico() -> Store {
    Store {
        usuarios: Mutex::new(HashMap::from([(
            "admin".to_string(),
            Usuario {
                id: 1,
                senha: "admin".to_string(),
                profiletype: "motorista".to_string(),
            },
        )])),
        nomes_por_id: Mutex::new(HashMap::from([(1, "admin".to_string())])),
        sessoes: Mutex::new(HashMap::new()),
        prox_token: AtomicU64::new(0),
        prox_user: AtomicU64::new(2),
        grafo: grafo_sintetico(),
        shapes: HashMap::new(),
        caronas: Mutex::new(crate::carona::CaronaStore::new()),
    }
}

/// Server com grafo sintético e usuários base.
fn server_sintetico() -> u16 {
    serve_com(LPC::com_registros(store_sintetico()))
}

/// Request síncrono via protocolo: encode → socket → framing → decode.
fn request(
    port: u16,
    method: Method,
    resource: &str,
    payload: Option<&str>,
    token: Option<&str>,
) -> Response {
    let mut metadata = Metadata::new();
    if let Some(token) = token {
        metadata.insert(AUTHORIZATION, token);
    }

    let request = Request::new(
        RequestHeader::new("LPC".into(), Resource::new(resource), 0, method),
        metadata,
        payload.map(Payload::from_str),
    );
    let bytes = request.encode().expect("encode");

    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.write_all(&bytes).expect("write");

    let mut reader = BufReader::new(stream);
    let mut start_line = String::new();
    reader.read_line(&mut start_line).expect("read_line");
    let header = ResponseHeader::parse(start_line.trim_end_matches('\n')).expect("header");
    let total = header.message_size as usize;
    assert!(total >= start_line.len(), "message_size menor que a start-line");
    let mut rest = vec![0u8; total - start_line.len()];
    reader.read_exact(&mut rest).expect("read_exact");
    let mut message = start_line;
    message.push_str(&String::from_utf8(rest).expect("utf8"));
    Response::parse(&message).expect("parse")
}

fn get(port: u16, resource: &str) -> Response {
    request(port, Method::GET, resource, None, None)
}

fn post(port: u16, resource: &str, payload: &str) -> Response {
    request(port, Method::POST, resource, Some(payload), None)
}

/// Login e devolve o token de sessão.
fn obter_token(port: u16, nome: &str, senha: &str) -> String {
    let payload = serde_json::json!({ "nome": nome, "senha": senha }).to_string();
    let response = post(port, ".login", &payload);
    assert_eq!(
        response.header.status,
        Status::Ok,
        "login falhou: {}",
        response.header.status.code()
    );
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    serde_json::from_str::<serde_json::Value>(payload)
        .expect("json")
        .get("token")
        .and_then(|token| token.as_str())
        .expect("token")
        .to_string()
}

/// Data atual + offset (dias) em AAAA-MM-DD (UTC, como o server).
fn data_iso(offset: i64) -> String {
    (Utc::now() + chrono::Duration::days(offset)).format("%Y-%m-%d").to_string()
}

/// Monta a mensagem LPC crua (message_size calculado), sem enviar.
fn formato_lpc(method: &str, resource: &str, payload: &str, token: Option<&str>) -> String {
    let meta = token.map(|t| format!("authorization: {t}\n")).unwrap_or_default();
    let base = format!("LPC {method} {resource} \n{meta}\n{payload}");
    let mut digits = 1;
    let size = loop {
        let total = base.len() + digits;
        if total.to_string().len() == digits {
            break total;
        }
        digits += 1;
    };
    format!("LPC {method} {resource} {size}\n{meta}\n{payload}")
}

// ─── concorrência (agora autenticada) ──────────────────────────────

#[test]
fn respostas_paralelas_mesmo_recurso() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");
    let threads: usize = 16;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let token = token.clone();
            std::thread::spawn(move || {
                barrier.wait();
                request(port, Method::GET, ".users", None, Some(&token))
            })
        })
        .collect();

    for handle in handles {
        let response = handle.join().expect("thread");
        assert_eq!(response.header.status, Status::Ok);
        assert_eq!(response.header.resource, Resource::new(".users"));
        let payload = response.payload.expect("payload");
        let payload = payload.as_str().expect("utf8");
        assert!(payload.contains("admin"), "payload inesperado: {payload}");
    }
}

#[test]
fn resources_diferentes_em_paralelo() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");
    let resources = [".users", ".municipios", ".grafo.cidades"];
    let barrier = Arc::new(Barrier::new(resources.len()));

    let handles: Vec<_> = resources
        .iter()
        .map(|resource| {
            let barrier = Arc::clone(&barrier);
            let token = token.clone();
            let resource = resource.to_string();
            std::thread::spawn(move || {
                barrier.wait();
                let response = request(port, Method::GET, &resource, None, Some(&token));
                (resource, response)
            })
        })
        .collect();

    for handle in handles {
        let (resource, response) = handle.join().expect("thread");
        assert_eq!(response.header.status, Status::Ok);
        assert_eq!(response.header.resource, Resource::new(&resource));
    }
}

#[test]
fn recurso_inexistente_404() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");
    let response = request(port, Method::GET, ".nao-existe", None, Some(&token));
    assert_eq!(response.header.status, Status::NotFound);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("não encontrado"), "payload: {payload}");
}

#[test]
fn mensagem_malformada_400() {
    let port = start_server();

    // message_size não numérico: 400 imediato na start-line
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.write_all(b"LPC GET .users abc\n\n").expect("write");
    let mut reader = BufReader::new(stream);
    let mut start_line = String::new();
    reader.read_line(&mut start_line).expect("read_line");
    let header = ResponseHeader::parse(start_line.trim_end_matches('\n')).expect("header");
    assert_eq!(header.status, Status::BadRequest);

    // message_size menor que a própria start-line: 400
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.write_all(b"LPC GET .users 10\n\n").expect("write");
    let mut reader = BufReader::new(stream);
    let mut start_line = String::new();
    reader.read_line(&mut start_line).expect("read_line");
    let header = ResponseHeader::parse(start_line.trim_end_matches('\n')).expect("header");
    assert_eq!(header.status, Status::BadRequest);
}

#[test]
fn queda_de_cliente_nao_derruba_o_server() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");

    // cliente conecta e morre sem enviar nada
    let s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    drop(s);
    // cliente envia só a metade da mensagem e morre
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.write_all(b"LPC POST .caronas ").expect("write parcial");
    drop(s);
    // cliente envia a mensagem inteira e morre antes de ler a resposta
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let payload = CARONA_JSON;
    let msg = formato_lpc("POST", ".caronas", &payload, None);
    s.write_all(msg.as_bytes()).expect("write");
    drop(s);

    // dá tempo das threads de conexão morrerem
    std::thread::sleep(Duration::from_millis(300));

    // o server continua servindo normalmente
    let response = request(port, Method::GET, ".users", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
    let response = request(port, Method::GET, ".municipios", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
}

#[test]
fn message_id_ecoado() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");

    let request = Request::new(
        RequestHeader::new("LPC".into(), Resource::new(".users"), 0, Method::GET),
        {
            let mut metadata = Metadata::new();
            metadata.insert(MESSAGE_ID, "teste-123");
            metadata.insert(AUTHORIZATION, &token);
            metadata
        },
        None,
    );
    let bytes = request.encode().expect("encode");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.write_all(&bytes).expect("write");

    let mut reader = BufReader::new(stream);
    let mut start_line = String::new();
    reader.read_line(&mut start_line).expect("read_line");
    let header = ResponseHeader::parse(start_line.trim_end_matches('\n')).expect("header");
    let total = header.message_size as usize;
    assert!(total >= start_line.len());
    let mut rest = vec![0u8; total - start_line.len()];
    reader.read_exact(&mut rest).expect("read_exact");
    let mut message = start_line;
    message.push_str(&String::from_utf8(rest).expect("utf8"));
    let response = Response::parse(&message).expect("parse");

    assert_eq!(response.metadata.get(MESSAGE_ID), Some("teste-123"));
}

// ─── autenticação ──────────────────────────────────────────────────

#[test]
fn login_emite_token() {
    let port = start_server();
    let response = post(port, ".login", r#"{"nome":"admin","senha":"admin"}"#);
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("token"), "sem token: {payload}");
}

#[test]
fn login_credenciais_invalidas() {
    let port = start_server();
    let response = post(port, ".login", r#"{"nome":"admin","senha":"errada"}"#);
    assert_eq!(response.header.status, Status::Unauthorized);
}

#[test]
fn recurso_protegido_sem_token() {
    let port = start_server();
    let response = get(port, ".municipios");
    assert_eq!(response.header.status, Status::Unauthorized);
}

#[test]
fn recurso_protegido_com_token() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");
    let response = request(port, Method::GET, ".municipios", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
}

#[test]
fn registro_de_usuario_pode_logar() {
    let port = start_server();
    let response = post(port, ".users", r#"{"nome":"maria","senha":"1234"}"#);
    assert_eq!(response.header.status, Status::Created);

    let token = obter_token(port, "maria", "1234");
    assert!(!token.is_empty());
}

#[test]
fn logout_invalida_token() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");

    let response = request(port, Method::POST, ".logout", Some("{}"), Some(&token));
    assert_eq!(response.header.status, Status::Ok);

    // token morto: recurso protegido agora devolve 401
    let response = request(port, Method::GET, ".users", None, Some(&token));
    assert_eq!(response.header.status, Status::Unauthorized);
}

#[test]
fn logout_sem_token_401() {
    let port = start_server();
    let response = post(port, ".logout", "{}");
    assert_eq!(response.header.status, Status::Unauthorized);
}

#[test]
fn recurso_composto_roteia_para_raiz() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");
    // `.users.123` casa com o handler registrado na raiz `.users`
    let response = request(port, Method::GET, ".users.123", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
}

#[test]
fn login_devolve_profiletype() {
    let port = start_server();
    let payload = serde_json::json!({ "nome": "admin", "senha": "admin" }).to_string();
    let response = post(port, ".login", &payload);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("motorista"), "sem profiletype: {payload}");
}

#[test]
fn registro_com_profiletype() {
    let port = start_server();
    let response = post(
        port,
        ".users",
        r#"{"nome":"mo","senha":"1234","profiletype":"motorista"}"#,
    );
    assert_eq!(response.header.status, Status::Created);

    // login devolve o profiletype registrado
    let payload = serde_json::json!({ "nome": "mo", "senha": "1234" }).to_string();
    let response = post(port, ".login", &payload);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("motorista"), "profiletype errado: {payload}");
}

// ─── grafo: rotas ─────────────────────────────────────────────────

#[test]
fn rota_curta_entre_municipios() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    let response = request(
        port,
        Method::GET,
        ".rota",
        Some(r#"{"origem":"Salvador","destino":"Feira de Santana"}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    let value: serde_json::Value = serde_json::from_str(payload).expect("json");
    assert_eq!(value["distancia_km"], 3.0);
    assert_eq!(
        value["municipios"],
        serde_json::json!(["Salvador", "Santo Amaro", "Feira de Santana"])
    );
    assert_eq!(value["trechos"].as_array().expect("trechos").len(), 3);
    // grafo sintético não tem geometry: linha/marcadores vazios mas presentes
    assert_eq!(value["linha"], serde_json::json!([]));
    assert_eq!(value["marcadores"], serde_json::json!([]));
}

#[test]
fn rota_com_via_obriga_passagem() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    let response = request(
        port,
        Method::GET,
        ".rota",
        Some(r#"{"origem":"Salvador","destino":"Feira de Santana","via":["Jequié"]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("Jequié"), "via ignorada: {payload}");
    assert!(payload.contains("\"distancia_km\":102.0"), "km errado: {payload}");
}

#[test]
fn rota_sem_caminho_404_e_payload_invalido_400() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    let response = request(
        port,
        Method::GET,
        ".rota",
        Some(r#"{"origem":"Xique-Xique","destino":"Salvador"}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::NotFound);
    let response = request(
        port,
        Method::GET,
        ".rota",
        Some(r#"{"origem":"Salvador"}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::BadRequest);
    let response = request(port, Method::GET, ".rota", None, Some(&token));
    assert_eq!(response.header.status, Status::BadRequest);
}

#[test]
fn grafo_cidades_e_segmento() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    let response = request(port, Method::GET, ".grafo.cidades", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("Salvador"));

    let response = request(port, Method::GET, ".grafo.salvador", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("BR-324"));

    let response = request(port, Method::GET, ".grafo.nao-existe", None, Some(&token));
    assert_eq!(response.header.status, Status::NotFound);

    let response = request(port, Method::GET, ".grafo", None, Some(&token));
    assert_eq!(response.header.status, Status::BadRequest);
}

#[test]
fn municipios_lista_e_ids_desconhecidos_404() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    let response = request(port, Method::GET, ".municipios", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("Santo Amaro"));

    // .municipios.malha foi removido: qualquer id desconhecido é 404
    for id in ["malha", "xyz"] {
        let response =
            request(port, Method::GET, &format!(".municipios.{id}"), None, Some(&token));
        assert_eq!(response.header.status, Status::NotFound, "id {id}");
    }
}

#[test]
fn municipios_shapes_filtrado_e_resto_404() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    // shapes vazio no sintético: responde Ok com objeto vazio
    let response = request(
        port,
        Method::GET,
        ".municipios.shapes",
        Some(r#"{"municipios":["Salvador","Inexistente"]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload").as_str().expect("utf8").to_string();
    let value: serde_json::Value = serde_json::from_str(&payload).expect("json");
    assert!(value["shapes"].as_object().expect("shapes").is_empty());

    // sem payload também responde objetos (vazios aqui)
    let response = request(port, Method::GET, ".municipios.shapes", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);

    // o resto dos ids continua 404
    let response = request(port, Method::GET, ".municipios.xyz", None, Some(&token));
    assert_eq!(response.header.status, Status::NotFound);
}

// ─── caronas: publicação ──────────────────────────────────────────

const CARONA_JSON: &str = r#"{
    "partida": "2099-01-01T08:00",
    "trechos": [
        {"origem":"Salvador","destino":"Santo Amaro","preco":1000,"assentos":2},
        {"origem":"Santo Amaro","destino":"Feira de Santana","preco":2000,"assentos":2}
    ]
}"#;

#[test]
fn publicar_e_listar_carona() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    let response = request(port, Method::POST, ".caronas", Some(CARONA_JSON), Some(&token));
    assert_eq!(response.header.status, Status::Created);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"preco\":2000"), "preço por trecho: {payload}");

    let response = request(port, Method::GET, ".caronas", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
    assert!(response.payload.expect("payload").as_str().expect("utf8").contains("Salvador"));

    let response = request(port, Method::GET, ".caronas.1", None, Some(&token));
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"km\":2.0"));
    assert!(payload.contains("\"assentos\":2"), "assentos por trecho: {payload}");

    let response = request(port, Method::GET, ".caronas.99", None, Some(&token));
    assert_eq!(response.header.status, Status::NotFound);
}

#[test]
fn carona_invalida_400() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");

    // trechos que não encadeiam
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(r#"{"partida":"2099-01-01T08:00","trechos":[{"origem":"Salvador","destino":"Santo Amaro","preco":1,"assentos":1},{"origem":"Salvador","destino":"Feira de Santana","preco":1,"assentos":1}]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::BadRequest);

    // município repetido
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(r#"{"partida":"2099-01-01T08:00","trechos":[{"origem":"Salvador","destino":"Santo Amaro","preco":1,"assentos":1},{"origem":"Santo Amaro","destino":"Salvador","preco":1,"assentos":1}]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::BadRequest);

    // par desconexo / município desconhecido
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(r#"{"partida":"2099-01-01T08:00","trechos":[{"origem":"Xique-Xique","destino":"Santo Amaro","preco":1,"assentos":1}]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::BadRequest);

    // partida inválida
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(r#"{"partida":"20/09/2026 08:00","trechos":[{"origem":"Salvador","destino":"Santo Amaro","preco":1,"assentos":1}]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::BadRequest);

    // preço negativo
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(r#"{"partida":"2099-01-01T08:00","trechos":[{"origem":"Salvador","destino":"Santo Amaro","preco":-1,"assentos":1}]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::BadRequest);

    // trecho sem 'assentos'
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(r#"{"partida":"2099-01-01T08:00","trechos":[{"origem":"Salvador","destino":"Santo Amaro","preco":1}]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::BadRequest);

    // assentos zero
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(r#"{"partida":"2099-01-01T08:00","trechos":[{"origem":"Salvador","destino":"Santo Amaro","preco":1,"assentos":0}]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::BadRequest);

    // partida no passado
    let passada = format!(
        r#"{{"partida":"{}T08:00","trechos":[{{"origem":"Salvador","destino":"Santo Amaro","preco":1,"assentos":1}}]}}"#,
        data_iso(-1)
    );
    let response = request(port, Method::POST, ".caronas", Some(&passada), Some(&token));
    assert_eq!(response.header.status, Status::BadRequest);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("passado"), "mensagem: {payload}");
}

#[test]
fn cancelar_carona_so_o_dono() {
    let port = server_sintetico();
    let token_motorista = obter_token(port, "admin", "admin");
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(CARONA_JSON),
        Some(&token_motorista),
    );
    assert_eq!(response.header.status, Status::Created);

    // passageira tenta cancelar: 403
    let payload = serde_json::json!({ "nome": "maria", "senha": "1234" }).to_string();
    let _ = post(port, ".users", &payload);
    let token_maria = obter_token(port, "maria", "1234");
    let response = request(
        port,
        Method::POST,
        ".caronas.1.cancelar",
        Some("{}"),
        Some(&token_maria),
    );
    assert_eq!(response.header.status, Status::Forbidden);

    // dono cancela: 200 e some da lista
    let response = request(
        port,
        Method::POST,
        ".caronas.1.cancelar",
        Some("{}"),
        Some(&token_motorista),
    );
    assert_eq!(response.header.status, Status::Ok);
    let response = request(port, Method::GET, ".caronas.1", None, Some(&token_motorista));
    assert_eq!(response.header.status, Status::NotFound);
}

// ─── corrida de assentos (concorrência) ────────────────────────────

#[test]
fn corrida_de_assentos_2_de_8_passageiros() {
    let port = server_sintetico();
    let token_motorista = obter_token(port, "admin", "admin");
    let response = request(
        port,
        Method::POST,
        ".caronas",
        Some(CARONA_JSON),
        Some(&token_motorista),
    );
    assert_eq!(response.header.status, Status::Created, "publicar carona");

    // 8 passageiros disputam 2 assentos da carona inteira
    let threads: usize = 8;
    let barrier = Arc::new(Barrier::new(threads));
    let inicio = std::time::Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            let nome = format!("passageiro{i}");
            std::thread::spawn(move || {
                let payload = serde_json::json!({ "nome": nome, "senha": "1234" }).to_string();
                let _ = request(port, Method::POST, ".users", Some(&payload), None);
                let token = obter_token(port, &nome, "1234");
                barrier.wait();
                let reserva = serde_json::json!({
                    "trechos": [
                        {"carona_id": 1, "embarque": "Salvador", "desembarque": "Feira de Santana"}
                    ]
                })
                .to_string();
                let response =
                    request(port, Method::POST, ".reservas", Some(&reserva), Some(&token));
                response.header.status
            })
        })
        .collect();

    let mut ok = 0;
    let mut conflito = 0;
    for handle in handles {
        match handle.join().expect("thread") {
            Status::Created => ok += 1,
            Status::Conflict => conflito += 1,
            outro => panic!("status inesperado: {outro:?}"),
        }
    }
    let elapsed = inicio.elapsed();
    println!("corrida de assentos: {ok} ok, {conflito} conflito em {elapsed:?}");

    assert_eq!(ok, 2, "exatamente 2 reservas cabem");
    assert_eq!(conflito, 6, "as outras 6 batem em 409");
    assert!(elapsed < std::time::Duration::from_secs(10), "corrida muito lenta: {elapsed:?}");

    // os 2 assentos estão ocupados nos 2 trechos
    let response = request(port, Method::GET, ".caronas.1", None, Some(&token_motorista));
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert_eq!(
        payload.matches("\"ocupados\":2").count(),
        2,
        "ocupados por trecho: {payload}"
    );
}

/// Dispara `n` reservas simultâneas (barreira) contra `carona_id` e
/// devolve (ok, conflito, latência média, latência máxima, tempo total, req/s).
fn corrida_de_reservas(
    port: u16,
    carona_id: u32,
    tokens: Vec<String>,
) -> (usize, usize, Duration, Duration, Duration, f64) {
    let n = tokens.len();
    let barrier = Arc::new(Barrier::new(n));
    let inicio = std::time::Instant::now();
    let handles: Vec<_> = tokens
        .into_iter()
        .map(|token| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let t0 = std::time::Instant::now();
                let reserva = serde_json::json!({
                    "trechos": [{"carona_id": carona_id, "embarque": "Salvador", "desembarque": "Feira de Santana"}]
                })
                .to_string();
                let response =
                    request(port, Method::POST, ".reservas", Some(&reserva), Some(&token));
                (response.header.status, t0.elapsed())
            })
        })
        .collect();

    let mut ok = 0;
    let mut conflito = 0;
    let mut soma = Duration::ZERO;
    let mut max = Duration::ZERO;
    for handle in handles {
        let (status, latencia) = handle.join().expect("thread");
        match status {
            Status::Created => ok += 1,
            Status::Conflict => conflito += 1,
            outro => panic!("status inesperado: {outro:?}"),
        }
        soma += latencia;
        max = max.max(latencia);
    }
    let total = inicio.elapsed();
    let media = soma / n as u32;
    let reqs_por_segundo = n as f64 / total.as_secs_f64();
    (ok, conflito, media, max, total, reqs_por_segundo)
}

/// Etapa B — experimento de carga para o relatório: corrida de assentos
/// com 8, 16 e 32 passageiros disputando 2 vagas (carona fresca por rodada).
#[test]
fn carga_corrida_de_assentos_mede_metricas() {
    let port = server_sintetico();
    let token_motorista = obter_token(port, "admin", "admin");
    let mut prox = 0;

    println!("\n── experimento: corrida de assentos (2 vagas por trecho) ──");
    println!("{:>4} | {:>4} | {:>4} | {:>9} | {:>10} | {:>10} | {:>8}", "N", "201", "409", "total", "lat média", "lat máx", "req/s");
    for n in [8usize, 16, 32] {
        // carona fresca por rodada e passageiros pré-registrados/logados
        let response = request(
            port,
            Method::POST,
            ".caronas",
            Some(CARONA_JSON),
            Some(&token_motorista),
        );
        assert_eq!(response.header.status, Status::Created);
        let payload = response.payload.expect("payload");
        let payload = payload.as_str().expect("utf8");
        let carona_id = serde_json::from_str::<serde_json::Value>(payload)
            .expect("json")["carona"]["id"]
            .as_u64()
            .expect("id") as u32;

        let mut tokens = Vec::with_capacity(n);
        for _ in 0..n {
            let nome = format!("carga{prox}");
            prox += 1;
            let registro = serde_json::json!({ "nome": nome, "senha": "1234" }).to_string();
            let _ = post(port, ".users", &registro);
            tokens.push(obter_token(port, &nome, "1234"));
        }

        let (ok, conflito, media, max, total, tps) =
            corrida_de_reservas(port, carona_id, tokens);
        println!(
            "{n:>4} | {ok:>4} | {conflito:>4} | {:>7.1?} | {media:>8.1?} | {max:>8.1?} | {tps:>6.0}",
            total
        );

        assert_eq!(ok, 2, "exatamente 2 vagas vendidas com N={n}");
        assert_eq!(conflito, n - 2, "todos os demais batem em 409 com N={n}");
        assert!(total < Duration::from_secs(10), "rodada N={n} muito lenta: {total:?}");
    }
}

#[test]
fn reserva_atomica_multi_carona_sem_commit_parcial() {
    let port = server_sintetico();
    let token_motorista = obter_token(port, "admin", "admin");

    // carona 1: Salvador -> Santo Amaro -> Feira (1 assento por trecho)
    let c1 = r#"{"partida":"2099-01-01T08:00","trechos":[{"origem":"Salvador","destino":"Santo Amaro","preco":1000,"assentos":1},{"origem":"Santo Amaro","destino":"Feira de Santana","preco":2000,"assentos":1}]}"#;
    let response = request(port, Method::POST, ".caronas", Some(c1), Some(&token_motorista));
    assert_eq!(response.header.status, Status::Created);
    // carona 2: Feira -> Jequié (1 assento)
    let c2 = r#"{"partida":"2099-01-01T10:00","trechos":[{"origem":"Feira de Santana","destino":"Jequié","preco":1500,"assentos":1}]}"#;
    let response = request(port, Method::POST, ".caronas", Some(c2), Some(&token_motorista));
    assert_eq!(response.header.status, Status::Created);

    let reserva_2_trechos = r#"{"trechos":[{"carona_id":1,"embarque":"Salvador","desembarque":"Feira de Santana"},{"carona_id":2,"embarque":"Feira de Santana","desembarque":"Jequié"}]}"#;

    // 3 passageiros disputam o mesmo itinerário (1 assento em cada carona)
    let threads: usize = 3;
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            let nome = format!("p{i}");
            std::thread::spawn(move || {
                let payload = serde_json::json!({ "nome": nome, "senha": "1234" }).to_string();
                let _ = request(port, Method::POST, ".users", Some(&payload), None);
                let token = obter_token(port, &nome, "1234");
                barrier.wait();
                let response = request(
                    port,
                    Method::POST,
                    ".reservas",
                    Some(reserva_2_trechos),
                    Some(&token),
                );
                response.header.status
            })
        })
        .collect();

    let mut ok = 0;
    let mut conflito = 0;
    for handle in handles {
        match handle.join().expect("thread") {
            Status::Created => ok += 1,
            Status::Conflict => conflito += 1,
            outro => panic!("status inesperado: {outro:?}"),
        }
    }
    assert_eq!(ok, 1, "1 reserva vence a disputa");
    assert_eq!(conflito, 2);

    // só a reserva vencedora ocupou assentos (nada de commit parcial)
    let response = request(port, Method::GET, ".caronas.1", None, Some(&token_motorista));
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert_eq!(payload.matches("\"ocupados\":1").count(), 2, "carona 1: {payload}");
    let response = request(port, Method::GET, ".caronas.2", None, Some(&token_motorista));
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert_eq!(payload.matches("\"ocupados\":1").count(), 1, "carona 2: {payload}");

    // a reserva vencedora aparece em .reservas com o total somado
    let response = request(port, Method::GET, ".reservas", None, Some(&token_motorista));
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"total_centavos\":4500"), "total da reserva: {payload}");

    // cancelar a reserva libera os assentos nas duas caronas
    let response = request(
        port,
        Method::POST,
        ".reservas.1.cancelar",
        Some("{}"),
        Some(&token_motorista),
    );
    assert_eq!(response.header.status, Status::Ok);
    let response = request(port, Method::GET, ".caronas.1", None, Some(&token_motorista));
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert_eq!(
        payload.matches("\"ocupados\":0").count(),
        2,
        "assentos liberados na carona 1: {payload}"
    );
}

#[test]
fn itinerarios_pela_rede() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    let c1 = r#"{"partida":"2099-01-01T08:00","trechos":[{"origem":"Salvador","destino":"Santo Amaro","preco":1000,"assentos":2},{"origem":"Santo Amaro","destino":"Feira de Santana","preco":2000,"assentos":2}]}"#;
    assert_eq!(request(port, Method::POST, ".caronas", Some(c1), Some(&token)).header.status, Status::Created);
    let c2 = r#"{"partida":"2099-01-01T10:00","trechos":[{"origem":"Feira de Santana","destino":"Jequié","preco":1500,"assentos":2}]}"#;
    assert_eq!(request(port, Method::POST, ".caronas", Some(c2), Some(&token)).header.status, Status::Created);

    let response = request(
        port,
        Method::GET,
        ".itinerarios",
        Some(r#"{"origem":"Salvador","destino":"Jequié","data":"2099-01-01"}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"total_centavos\":4500"), "itinerário: {payload}");
    assert!(payload.contains("\"partida\":\"2099-01-01T08:00\""), "partida do 1º trecho: {payload}");
    assert!(payload.contains("\"carona_id\":2"), "2º trecho: {payload}");
    assert!(payload.contains("\"assentos_disponiveis\":2"), "lotação restante: {payload}");

    // outro dia: nada
    let response = request(
        port,
        Method::GET,
        ".itinerarios",
        Some(r#"{"origem":"Salvador","destino":"Jequié","data":"2099-01-02"}"#),
        Some(&token),
    );
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert_eq!(payload, r#"{"itinerarios":[]}"#);

    // data no passado: 400
    let passada = format!(
        r#"{{"origem":"Salvador","destino":"Jequié","data":"{}"}}"#,
        data_iso(-1)
    );
    let response = request(port, Method::GET, ".itinerarios", Some(&passada), Some(&token));
    assert_eq!(response.header.status, Status::BadRequest);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("passado"), "mensagem: {payload}");
}

#[test]
fn rota_real_salvador_feira_com_dados_do_grafo() {
    // usa LPC::new() → carrega data/ba_grafo.json (quando presente)
    let port = start_server();
    let token = obter_token(port, "admin", "admin");
    let response = request(
        port,
        Method::GET,
        ".rota",
        Some(r#"{"origem":"Salvador","destino":"Feira de Santana"}"#),
        Some(&token),
    );
    if response.header.status == Status::NotFound {
        eprintln!("aviso: grafo real ausente; teste de dados reais ignorado");
        return;
    }
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    let value: serde_json::Value = serde_json::from_str(payload).expect("json");
    let km = value["distancia_km"].as_f64().expect("km");
    assert!(km > 50.0, "Salvador->Feira deveria ter dezenas de km: {km}");
    let municipios = value["municipios"].as_array().expect("municipios");
    assert_eq!(municipios.first(), Some(&serde_json::Value::String("Salvador".into())));
    assert_eq!(
        municipios.last(),
        Some(&serde_json::Value::String("Feira de Santana".into()))
    );
    // polyline com geometria real: não-vazia, com marcador em cada ponta
    let linha = value["linha"].as_array().expect("linha");
    if linha.is_empty() {
        eprintln!("aviso: grafo real sem geometry; linha vazia ignorada");
        return;
    }
    assert!(linha.len() > 2, "linha deveria ter vários pontos: {}", linha.len());
    let marcadores = value["marcadores"].as_array().expect("marcadores");
    assert!(marcadores.len() >= 2, "marcadores deveriam incluir pontas");
    assert_eq!(marcadores[0]["nome"], "Salvador");
    assert_eq!(marcadores[marcadores.len() - 1]["nome"], "Feira de Santana");
    for marcador in marcadores {
        let coord = marcador["coord"].as_array().expect("coord");
        assert_eq!(coord.len(), 2);
        assert!(coord[0].as_f64().expect("lng").abs() < 90.0, "lng fora de faixa");
        assert!(coord[1].as_f64().expect("lat").abs() < 90.0, "lat fora de faixa");
    }
}

#[test]
fn rota_real_longa_linha_completa_e_payload_razoavel() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");
    let response = request(
        port,
        Method::GET,
        ".rota",
        Some(r#"{"origem":"Salvador","destino":"Barreiras"}"#),
        Some(&token),
    );
    if response.header.status == Status::NotFound {
        eprintln!("aviso: grafo real ausente; teste de dados reais ignorado");
        return;
    }
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload").as_str().expect("utf8").to_string();
    // ~700 km de rota: a polyline inteira cabe folgada num payload pequeno
    assert!(payload.len() < 2_000_000, "payload grande demais: {} bytes", payload.len());
    let value: serde_json::Value = serde_json::from_str(&payload).expect("json");
    let linha = value["linha"].as_array().expect("linha");
    assert!(linha.len() > 10, "linha deveria ter muitos pontos: {}", linha.len());
    let todos_dupla: bool = linha.iter().all(|p| p.as_array().is_some_and(|c| c.len() == 2));
    assert!(todos_dupla, "todo ponto da linha deve ser [lng, lat]");
}

#[test]
fn municipios_shapes_reais_quando_arquivo_presente() {
    let port = start_server();
    let token = obter_token(port, "admin", "admin");
    let response = request(
        port,
        Method::GET,
        ".municipios.shapes",
        Some(r#"{"municipios":["Salvador","Inexistente"]}"#),
        Some(&token),
    );
    assert_eq!(response.header.status, Status::Ok);
    let payload = response.payload.expect("payload").as_str().expect("utf8").to_string();
    let value: serde_json::Value = serde_json::from_str(&payload).expect("json");
    let shapes = value["shapes"].as_object().expect("shapes");
    if shapes.is_empty() {
        eprintln!("aviso: arquivo de shapes ausente; teste ignorado");
        return;
    }
    assert!(shapes.contains_key("Salvador"), "Salvador deveria ter shape");
    assert!(!shapes.contains_key("Inexistente"), "pedido desconhecido deve ser ignorado");
    let shape = &shapes["Salvador"];
    assert_eq!(shape["type"], "Polygon");
    let anel = shape["coordinates"].as_array().expect("coordinates").first();
    assert!(anel.is_some(), "shape deveria ter ao menos um anel");
}


#[test]
fn registro_com_nome_duplicado_409() {
    let port = start_server();
    // admin já existe desde a carga inicial
    let response = post(port, ".users", r#"{"nome":"admin","senha":"outra"}"#);
    assert_eq!(response.header.status, Status::Conflict);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("já existe"), "mensagem: {payload}");

    // a senha do admin não foi sobrescrita
    let response = post(port, ".login", r#"{"nome":"admin","senha":"admin"}"#);
    assert_eq!(response.header.status, Status::Ok);
}

#[test]
fn login_devolve_id_e_registro_incrementa() {
    let port = start_server();
    // admin tem id 1
    let response = post(port, ".login", r#"{"nome":"admin","senha":"admin"}"#);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"id\":1"), "id do admin: {payload}");

    // registros recebem ids sequenciais 2, 3…
    let response = post(port, ".users", r#"{"nome":"maria","senha":"1234"}"#);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"id\":2"), "id da maria: {payload}");
    let response = post(port, ".users", r#"{"nome":"joao","senha":"1234"}"#);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"id\":3"), "id do joao: {payload}");
}

#[test]
fn carona_e_reserva_referenciam_dono_por_id() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin");
    let response = request(port, Method::POST, ".caronas", Some(CARONA_JSON), Some(&token));
    assert_eq!(response.header.status, Status::Created);

    // carona registra o motorista por id, não por nome
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"dono\":1"), "dono por id: {payload}");
    assert!(!payload.contains("\"dono\":\"admin\""), "dono não pode ser o nome: {payload}");
    // o nome viaja à parte, só para exibição
    assert!(payload.contains("\"dono_nome\":\"admin\""), "nome do dono: {payload}");

    // passageiro registra e reserva; reserva registra o dono por id
    let payload = serde_json::json!({ "nome": "maria", "senha": "1234" }).to_string();
    let _ = post(port, ".users", &payload);
    let token_maria = obter_token(port, "maria", "1234");
    let reserva = r#"{"trechos":[{"carona_id":1,"embarque":"Salvador","desembarque":"Feira de Santana"}]}"#;
    let response = request(port, Method::POST, ".reservas", Some(reserva), Some(&token_maria));
    assert_eq!(response.header.status, Status::Created);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("\"dono\":2"), "reserva com dono id 2: {payload}");
    assert!(payload.contains("\"dono_nome\":\"maria\""), "nome da passageira: {payload}");
}


#[test]
fn motorista_nao_pode_reservar_403() {
    let port = server_sintetico();
    let token = obter_token(port, "admin", "admin"); // motorista
    let response = request(port, Method::POST, ".caronas", Some(CARONA_JSON), Some(&token));
    assert_eq!(response.header.status, Status::Created);

    // o motorista tenta reservar a própria carona (e qualquer outra): 403
    let reserva = r#"{"trechos":[{"carona_id":1,"embarque":"Salvador","desembarque":"Feira de Santana"}]}"#;
    let response = request(port, Method::POST, ".reservas", Some(reserva), Some(&token));
    assert_eq!(response.header.status, Status::Forbidden);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(
        payload.contains("apenas passageiros"),
        "mensagem da separação: {payload}"
    );
    // e nenhum assento foi ocupado
    let response = request(port, Method::GET, ".caronas.1", None, Some(&token));
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert_eq!(payload.matches("\"ocupados\":0").count(), 2, "vagas intactas: {payload}");
}

#[test]
fn passageiro_nao_publica_carona_403() {
    let port = server_sintetico();
    let payload = serde_json::json!({ "nome": "maria", "senha": "1234" }).to_string();
    let _ = post(port, ".users", &payload); // passageiro (default)
    let token = obter_token(port, "maria", "1234");

    let response = request(port, Method::POST, ".caronas", Some(CARONA_JSON), Some(&token));
    assert_eq!(response.header.status, Status::Forbidden);
    let payload = response.payload.expect("payload");
    let payload = payload.as_str().expect("utf8");
    assert!(payload.contains("apenas motoristas"), "mensagem: {payload}");

    // mas ela reserva normalmente
    let motorista = obter_token(port, "admin", "admin");
    let response = request(port, Method::POST, ".caronas", Some(CARONA_JSON), Some(&motorista));
    assert_eq!(response.header.status, Status::Created);
    let reserva = r#"{"trechos":[{"carona_id":1,"embarque":"Salvador","desembarque":"Feira de Santana"}]}"#;
    let response = request(port, Method::POST, ".reservas", Some(reserva), Some(&token));
    assert_eq!(response.header.status, Status::Created);
}
