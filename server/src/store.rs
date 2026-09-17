//! Estado central do servidor: contas, sessões, grafo rodoviário e shapes
//! dos municípios, além de validações de entrada (datas via `chrono`; km e coords arredondados).
//!
//! O grafo e os shapes são carregados uma única vez no boot e ficam imutáveis
//! (leitura concorrente sem lock); o que muda (usuários, sessões, caronas e
//! reservas) vive atrás de `Mutex`.

use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{NaiveDate, NaiveDateTime, Utc};

use ligerin_protocol::{AUTHORIZATION, Request};

use crate::carona::CaronaStore;
use crate::grafo::Grafo;

pub(crate) struct Usuario {
    /// Id único do usuário (referência em caronas/reservas, em vez do nome).
    pub(crate) id: u32,
    pub(crate) senha: String,
    pub(crate) profiletype: String, // "motorista" | "passageiro"
}

/// Estado em memória do server (compartilhado entre threads).
pub(crate) struct Store {
    pub(crate) usuarios: Mutex<HashMap<String, Usuario>>,
    /// Índice id -> nome (espelho de `usuarios`, consulta O(1) no JSON).
    pub(crate) nomes_por_id: Mutex<HashMap<u32, String>>,
    pub(crate) sessoes: Mutex<HashMap<String, (u32, String)>>, // token -> (id, profiletype)
    pub(crate) prox_token: AtomicU64,
    pub(crate) prox_user: AtomicU64,
    /// Grafo rodoviário (imutável após a carga; leitura sem lock).
    pub(crate) grafo: Grafo,
    /// Shape (polígono) por município para o mapa offline (imutável).
    pub(crate) shapes: HashMap<String, serde_json::Value>,
    pub(crate) caronas: Mutex<CaronaStore>,
}

impl Store {
    pub(crate) fn new() -> Self {
        let mut usuarios = HashMap::new();
        usuarios.insert(
            "admin".to_string(),
            Usuario {
                id: 1,
                senha: "admin".to_string(),
                profiletype: "motorista".to_string(),
            },
        );

        Self {
            usuarios: Mutex::new(usuarios),
            nomes_por_id: Mutex::new(HashMap::from([(1, "admin".to_string())])),
            sessoes: Mutex::new(HashMap::new()),
            prox_token: AtomicU64::new(0),
            prox_user: AtomicU64::new(2), // admin é o id 1; registros seguem do 2
            grafo: Grafo::carregar_auto(),
            shapes: carregar_shapes_auto(),
            caronas: Mutex::new(CaronaStore::new()),
        }
    }

    pub(crate) fn nome_por_id(&self, id: u32) -> Option<String> {
        self.nomes_por_id.lock().expect("lock").get(&id).cloned()
    }

    /// Valida credenciais e emite um token de sessão; devolve (token, profiletype, id).
    pub(crate) fn autenticar(&self, nome: &str, senha: &str) -> Option<(String, String, u32)> {
        let usuarios = self.usuarios.lock().expect("lock");
        let Some(usuario) = usuarios.get(nome) else {
            return None;
        };
        if usuario.senha != senha {
            return None;
        }
        let id = usuario.id;
        let profiletype = usuario.profiletype.clone();
        drop(usuarios);

        let token = format!(
            "{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            self.prox_token.fetch_add(1, Ordering::Relaxed)
        );
        self.sessoes.lock().expect("lock").insert(token.clone(), (id, profiletype.clone()));
        Some((token, profiletype, id))
    }

    pub(crate) fn token_valido(&self, token: &str) -> bool {
        self.sessoes.lock().expect("lock").contains_key(token)
    }

    /// Encerra a sessão do token.
    pub(crate) fn revogar(&self, token: &str) {
        self.sessoes.lock().expect("lock").remove(token);
    }
}

/// Lê os shapes por município (payload de `.municipios.shapes`): mesmo arquivo
/// do grafo por município, mas só o campo `shape` de cada entrada importa.
fn carregar_shapes_auto() -> HashMap<String, serde_json::Value> {
    let caminhos: Vec<String> = match std::env::var("LIGERIN_SHAPES") {
        Ok(caminho) if !caminho.is_empty() => vec![caminho],
        _ => vec![
            "data/ba_grafo_por_municipio.json".into(),
            "../data/ba_grafo_por_municipio.json".into(),
        ],
    };
    for caminho in caminhos {
        let Ok(texto) = fs::read_to_string(&caminho) else { continue };
        let Ok(serde_json::Value::Object(raiz)) = serde_json::from_str::<serde_json::Value>(&texto)
        else {
            continue;
        };
        let mut shapes = HashMap::new();
        for (municipio, v) in raiz {
            if let Some(shape) = v.get("shape") {
                shapes.insert(municipio, shape.clone());
            }
        }
        return shapes;
    }
    eprintln!("aviso: ba_grafo_por_municipio.json não encontrado; shapes vazio");
    HashMap::new()
}

/// (id, profiletype) do dono do token da requisição (sessão já validada).
pub(crate) fn sessao_atual(request: &Request, store: &Store) -> Option<(u32, String)> {
    let token = request.metadata.get(AUTHORIZATION)?;
    store.sessoes.lock().expect("lock").get(token).cloned()
}

/// Id do usuário dono do token da requisição (sessão já validada).
pub(crate) fn dono_atual(request: &Request, store: &Store) -> Option<u32> {
    sessao_atual(request, store).map(|(id, _)| id)
}

/// Profiletype do dono do token (separação de perfis: motorista publica,
/// passageiro reserva).
pub(crate) fn perfil_atual(request: &Request, store: &Store) -> Option<String> {
    sessao_atual(request, store).map(|(_, perfil)| perfil)
}

/// Valida "AAAA-MM-DDTHH:MM" (partida de carona), com calendário real
/// (rejeita datas inexistentes, ex.: 31/02) via chrono.
pub(crate) fn partida_valida(s: &str) -> bool {
    s.len() == 16 && NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").is_ok()
}

/// Valida "AAAA-MM-DD" (data de busca de itinerários), com calendário real.
pub(crate) fn data_valida(s: &str) -> bool {
    s.len() == 10 && NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

/// Arredonda km para 2 casas (evita 73.30000000000001 no payload).
pub(crate) fn arredondar(km: f64) -> f64 {
    (km * 100.0).round() / 100.0
}

/// Arredonda uma coordenada [lng, lat] para 5 casas (≈1 m; encurta o payload).
pub(crate) fn arredondar_coord(coord: [f64; 2]) -> [f64; 2] {
    let r = |v: f64| (v * 100_000.0).round() / 100_000.0;
    [r(coord[0]), r(coord[1])]
}

/// Data atual em AAAA-MM-DD (UTC).
pub(crate) fn hoje_iso() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}
