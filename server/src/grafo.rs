//! Grafo rodoviário da Bahia.
//!
//! Carrega `data/ba_grafo.json` (segmentos com `mun`, `rodovia`,
//! `extensao_km` e `conec`) e calcula rotas entre municípios:
//! **Dijkstra** (menor quilometragem) quando todos os segmentos têm extensão;
//! **BFS** (menos trechos) como fallback defensivo.
//!
//! O grafo é imutável depois de carregado: vive solto no `Store`, sem `Mutex`,
//! e as threads de conexão só fazem leitura.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

/// Um nó do grafo: um segmento de rodovia.
#[derive(Debug, Clone)]
pub struct No {
    pub mun: Vec<String>,
    pub rodovia: Option<String>,
    pub extensao_km: Option<f64>,
    /// Traçado do segmento ([lng, lat], GeoJSON); 
    pub geometry: Option<Vec<[f64; 2]>>,
    pub conec: Vec<String>,
}

/// Segmento físico de uma rota.
#[derive(Debug, Clone)]
pub struct TrechoFisico {
    pub id: String,
    pub mun: Vec<String>,
    pub rodovia: Option<String>,
    pub extensao_km: Option<f64>,
}

/// Parada nomeada da rota: município + coordenada de referência no mapa.
#[derive(Debug, Clone)]
pub struct Marcador {
    pub nome: String,
    pub coord: [f64; 2],
}

/// Rota calculada entre dois municípios.
#[derive(Debug, Clone)]
pub struct Rota {
    pub origem: String,
    pub destino: String,
    /// Soma das extensões dos segmentos do caminho, incluindo a partida.
    pub distancia_km: f64,
    /// Municípios atravessados, em ordem (duplicatas consecutivas removidas).
    pub municipios: Vec<String>,
    /// Segmentos físicos do caminho, em ordem.
    pub trechos: Vec<TrechoFisico>,
    /// Polyline única do percurso ([lng, lat]), concatenando as geometries dos
    /// segmentos (ponto de junção deduplicado).
    pub linha: Vec<[f64; 2]>,
    /// Paradas (municípios) com a coordenada do 1º ponto do segmento que os contém.
    pub marcadores: Vec<Marcador>,
}

pub struct Grafo {
    nos: HashMap<String, No>,
    /// Nome do município -> ids dos segmentos que passam por ele.
    cidade_nos: HashMap<String, Vec<String>>,
    /// `true` quando todos os nós têm `extensao_km` (rota por menor km).
    ponderado: bool,
}

impl Grafo {
    /// Grafo vazio — o server continua de pé mesmo sem o arquivo de dados.
    pub fn vazio() -> Self {
        Self { nos: HashMap::new(), cidade_nos: HashMap::new(), ponderado: false }
    }

    /// Constrói o grafo a partir dos nós (usado nos testes sintéticos).
    pub fn novo(nos: HashMap<String, No>) -> Self {
        let mut cidade_nos: HashMap<String, Vec<String>> = HashMap::new();
        let mut ponderado = true;
        for (id, no) in &nos {
            for mun in &no.mun {
                cidade_nos.entry(mun.clone()).or_default().push(id.clone());
            }
            if no.extensao_km.is_none() {
                ponderado = false;
            }
        }
        Self { nos, cidade_nos, ponderado }
    }

    /// Carrega do JSON ; arquivo ausente ou inválido -> grafo vazio com aviso.
    pub fn carregar(caminho: &Path) -> Self {
        let texto = match fs::read_to_string(caminho) {
            Ok(texto) => texto,
            Err(err) => {
                eprintln!("aviso: grafo não carregado de {}: {err}", caminho.display());
                return Self::vazio();
            }
        };
        match serde_json::from_str::<serde_json::Value>(&texto) {
            Ok(serde_json::Value::Object(raiz)) => {
                let mut nos = HashMap::new();
                for (id, v) in raiz {
                    let mun = v
                        .get("mun")
                        .and_then(|m| m.as_array())
                        .map(|a| a.iter().filter_map(|m| m.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    let rodovia = v.get("rodovia").and_then(|r| r.as_str()).map(String::from);
                    let extensao_km = v.get("extensao_km").and_then(|e| e.as_f64());
                    let conec = v
                        .get("conec")
                        .and_then(|c| c.as_array())
                        .map(|a| a.iter().filter_map(|c| c.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    let geometry = v
                        .get("geometry")
                        .and_then(|g| g.get("coordinates"))
                        .and_then(|c| c.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|p| {
                                    let x = p.get(0)?.as_f64()?;
                                    let y = p.get(1)?.as_f64()?;
                                    Some([x, y])
                                })
                                .collect()
                        })
                        .filter(|g: &Vec<[f64; 2]>| !g.is_empty());
                    nos.insert(id, No { mun, rodovia, extensao_km, geometry, conec });
                }
                let sem_extensao = nos.values().filter(|n| n.extensao_km.is_none()).count();
                if sem_extensao > 0 {
                    eprintln!("aviso: {sem_extensao} segmentos sem extensao_km; rotas por BFS");
                }
                Self::novo(nos)
            }
            Ok(_) => {
                eprintln!("aviso: grafo em formato inesperado");
                Self::vazio()
            }
            Err(err) => {
                eprintln!("aviso: grafo inválido: {err}");
                Self::vazio()
            }
        }
    }

    /// Candidatos de arquivo: env `LIGERIN_GRAFO`, depois caminhos locais.
    pub fn carregar_auto() -> Self {
        if let Ok(caminho) = std::env::var("LIGERIN_GRAFO") {
            if !caminho.is_empty() {
                return Self::carregar(Path::new(&caminho));
            }
        }
        for candidato in ["data/ba_grafo.json", "../data/ba_grafo.json"] {
            if Path::new(candidato).exists() {
                return Self::carregar(Path::new(candidato));
            }
        }
        eprintln!("aviso: ba_grafo.json não encontrado; grafo vazio");
        Self::vazio()
    }

    /// Nomes de todos os municípios que o grafo conhece, ordenados.
    pub fn cidades(&self) -> Vec<String> {
        let mut nomes: Vec<String> = self.cidade_nos.keys().cloned().collect();
        nomes.sort();
        nomes
    }

    pub fn no(&self, id: &str) -> Option<&No> {
        self.nos.get(id)
    }

    pub fn num_nos(&self) -> usize {
        self.nos.len()
    }

    /// Rota mais curta entre dois municípios.
    pub fn rota(&self, origem: &str, destino: &str) -> Option<Rota> {
        if origem == destino {
            return None;
        }
        let caminho = self.caminho(origem, destino)?;
        Some(self.montar_rota(origem, destino, caminho))
    }

    /// Rota passando por municípios intermediários (`via`), na ordem dada.
    pub fn rota_com_via(&self, origem: &str, destino: &str, via: &[String]) -> Option<Rota> {
        if origem == destino {
            return None;
        }
        let mut pontos: Vec<&str> = Vec::with_capacity(via.len() + 2);
        pontos.push(origem);
        pontos.extend(via.iter().map(String::as_str));
        pontos.push(destino);

        // encadeia o caminho de cada par consecutivo
        let mut caminho: Vec<String> = Vec::new();
        for par in pontos.windows(2) {
            let trecho = self.caminho(par[0], par[1])?;
            if caminho.is_empty() {
                caminho = trecho;
            } else {
                // o primeiro nó do trecho seguinte é o último do anterior
                caminho.extend(trecho.into_iter().skip(1));
            }
        }
        Some(self.montar_rota(origem, destino, caminho))
    }

    fn caminho(&self, origem: &str, destino: &str) -> Option<Vec<String>> {
        if self.ponderado {
            self.dijkstra(origem, destino)
        } else {
            self.bfs(origem, destino)
        }
    }

    /// Menor quilometragem: o custo da aresta é a extensão do segmento de
    /// destino. Multi-fonte (todos os segmentos do município de origem) e
    /// para no primeiro segmento do município de destino.
    fn dijkstra(&self, origem: &str, destino: &str) -> Option<Vec<String>> {
        let iniciais = self.cidade_nos.get(origem)?;
        let alvos: HashSet<&String> = self.cidade_nos.get(destino)?.iter().collect();

        let mut dist: HashMap<&String, f64> = HashMap::new();
        let mut prev: HashMap<String, String> = HashMap::new();
        let mut fila = BinaryHeap::new();
        for s in iniciais {
            dist.insert(s, 0.0);
            fila.push(Item { custo: 0.0, no: s.clone() });
        }

        while let Some(Item { custo, no }) = fila.pop() {
            // entrada obsoleta: já achamos caminho mais barato para `no`
            if dist.get(&no).map_or(true, |d| *d < custo) {
                continue;
            }
            if alvos.contains(&no) {
                return Some(reconstruir(&no, &prev));
            }
            let Some(no_dados) = self.nos.get(&no) else { continue };
            for vizinho in &no_dados.conec {
                let Some(viz_dados) = self.nos.get(vizinho) else { continue };
                let Some(ext) = viz_dados.extensao_km else { continue };
                let novo = custo + ext;
                if dist.get(vizinho).map_or(true, |d| novo < *d) {
                    dist.insert(vizinho, novo);
                    prev.insert(vizinho.clone(), no.clone());
                    fila.push(Item { custo: novo, no: vizinho.clone() });
                }
            }
        }
        None
    }

    /// Menos trechos (grafo sem pesos): BFS também multi-fonte.
    fn bfs(&self, origem: &str, destino: &str) -> Option<Vec<String>> {
        let iniciais = self.cidade_nos.get(origem)?;
        let alvos: HashSet<&String> = self.cidade_nos.get(destino)?.iter().collect();

        let mut prev: HashMap<String, String> = HashMap::new();
        let mut fila: VecDeque<String> = VecDeque::new();
        for s in iniciais {
            prev.insert(s.clone(), s.clone()); // marca visitado (self-loop)
            fila.push_back(s.clone());
        }
        while let Some(no) = fila.pop_front() {
            if alvos.contains(&no) {
                return Some(reconstruir(&no, &prev));
            }
            let Some(no_dados) = self.nos.get(&no) else { continue };
            for vizinho in &no_dados.conec {
                if !prev.contains_key(vizinho) {
                    prev.insert(vizinho.clone(), no.clone());
                    fila.push_back(vizinho.clone());
                }
            }
        }
        None
    }

    fn montar_rota(&self, origem: &str, destino: &str, caminho: Vec<String>) -> Rota {
        let mut municipios: Vec<String> = Vec::new();
        let mut trechos: Vec<TrechoFisico> = Vec::new();
        let mut linha: Vec<[f64; 2]> = Vec::new();
        let mut marcadores: Vec<Marcador> = Vec::new();
        let mut distancia_km = 0.0;
        for id in caminho {
            if let Some(no) = self.nos.get(&id) {
                distancia_km += no.extensao_km.unwrap_or(0.0);
                for mun in &no.mun {
                    if municipios.last() != Some(mun) {
                        municipios.push(mun.clone());
                        // parada: 1º ponto do 1º segmento que contém o município
                        if let Some(primeiro) = no.geometry.as_ref().and_then(|g| g.first()) {
                            marcadores.push(Marcador { nome: mun.clone(), coord: *primeiro });
                        }
                    }
                }
                // polyline única: junta os traçados, sem repetir o ponto de emenda
                if let Some(geometry) = &no.geometry {
                    match (linha.last(), geometry.first()) {
                        (Some(ultimo), Some(primeiro)) if ultimo == primeiro => {
                            linha.extend_from_slice(&geometry[1..]);
                        }
                        _ => linha.extend_from_slice(geometry),
                    }
                }
                trechos.push(TrechoFisico {
                    id,
                    mun: no.mun.clone(),
                    rodovia: no.rodovia.clone(),
                    extensao_km: no.extensao_km,
                });
            }
        }
        Rota {
            origem: origem.to_string(),
            destino: destino.to_string(),
            distancia_km,
            municipios,
            trechos,
            linha,
            marcadores,
        }
    }
}

/// Item da fila de prioridade: menor custo primeiro .
struct Item {
    custo: f64,
    no: String,
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.custo == other.custo
    }
}
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        other.custo.total_cmp(&self.custo)
    }
}

/// Reconstrói o caminho do nó alvo até a origem via `prev`.
fn reconstruir(alvo: &str, prev: &HashMap<String, String>) -> Vec<String> {
    let mut caminho = vec![alvo.to_string()];
    let mut atual = alvo;
    while let Some(anterior) = prev.get(atual) {
        if anterior == atual {
            break; // nó inicial (self-loop da marcação)
        }
        caminho.push(anterior.clone());
        atual = anterior;
    }
    caminho.reverse();
    caminho
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grafo sintético: Salvador->Santo Amaro->Feira é curto (3 km);
    /// Salvador->Jequié->Feira é longo (102 km) mas com o mesmo nº de trechos.
    /// Sem extensões, o BFS pode escolher qualquer um dos dois.
    fn sintetico(com_extensao: bool) -> Grafo {
        let km = |v: f64| if com_extensao { Some(v) } else { None };
        let nos = HashMap::from([
            (
                "salvador".to_string(),
                No {
                    mun: vec!["Salvador".into()],
                    rodovia: Some("BR-324".into()),
                    extensao_km: km(1.0),
                    geometry: None,
                    conec: vec!["santo_amaro".into(), "jequie".into()],
                },
            ),
            (
                "santo_amaro".to_string(),
                No {
                    mun: vec!["Santo Amaro".into()],
                    rodovia: Some("BR-324".into()),
                    extensao_km: km(1.0),
                    geometry: None,
                    conec: vec!["salvador".into(), "feira".into()],
                },
            ),
            (
                "jequie".to_string(),
                No {
                    mun: vec!["Jequié".into()],
                    rodovia: Some("BR-116".into()),
                    extensao_km: km(100.0),
                    geometry: None,
                    conec: vec!["salvador".into(), "feira".into()],
                },
            ),
            (
                "feira".to_string(),
                No {
                    mun: vec!["Feira de Santana".into()],
                    rodovia: Some("BR-324".into()),
                    extensao_km: km(1.0),
                    geometry: None,
                    conec: vec!["santo_amaro".into(), "jequie".into()],
                },
            ),
        ]);
        Grafo::novo(nos)
    }

    #[test]
    fn dijkstra_escolhe_menor_km_mesmo_custo_de_trechos() {
        let grafo = sintetico(true);
        let rota = grafo.rota("Salvador", "Feira de Santana").expect("rota");
        // Salvador(1) + Santo Amaro(1) + Feira(1) = 3 km; via Jequié seria 102
        assert_eq!(rota.distancia_km, 3.0);
        assert_eq!(rota.municipios, vec!["Salvador", "Santo Amaro", "Feira de Santana"]);
        assert_eq!(rota.trechos.len(), 3);
        assert!(rota.trechos.iter().all(|t| t.rodovia.is_some()));
    }

    #[test]
    fn bfs_usado_quando_falta_extensao() {
        let grafo = sintetico(false);
        let rota = grafo.rota("Salvador", "Feira de Santana").expect("rota");
        // sem pesos, os dois caminhos têm 2 trechos; não pode falhar
        assert_eq!(rota.trechos.len(), 3); // qualquer caminho tem 3 nós
        assert!(rota.distancia_km >= 0.0);
    }

    #[test]
    fn rota_com_via_encaixa_jequie_no_caminho() {
        let grafo = sintetico(true);
        let rota = grafo
            .rota_com_via("Salvador", "Feira de Santana", &["Jequié".to_string()])
            .expect("rota");
        assert_eq!(rota.municipios, vec!["Salvador", "Jequié", "Feira de Santana"]);
        assert_eq!(rota.distancia_km, 102.0);
    }

    #[test]
    fn rota_origem_destino_iguais_ou_desconhecidos() {
        let grafo = sintetico(true);
        assert!(grafo.rota("Salvador", "Salvador").is_none());
        assert!(grafo.rota("Salvador", "Xique-Xique").is_none());
        assert!(grafo.rota("Xique-Xique", "Salvador").is_none());
    }

    #[test]
    fn cidades_ordenadas_e_consulta_de_no() {
        let grafo = sintetico(true);
        assert_eq!(
            grafo.cidades(),
            vec!["Feira de Santana", "Jequié", "Salvador", "Santo Amaro"]
        );
        assert!(grafo.no("salvador").is_some());
        assert!(grafo.no("inexistente").is_none());
    }

    #[test]
    fn carregar_arquivo_ausente_vira_grafo_vazio() {
        let grafo = Grafo::carregar(Path::new("/tmp/nao-existe.json"));
        assert_eq!(grafo.num_nos(), 0);
        assert!(grafo.cidades().is_empty());
    }

    #[test]
    fn linha_concatenada_e_marcadores_nas_paradas() {
        let nos = HashMap::from([
            (
                "a".to_string(),
                No {
                    mun: vec!["Salvador".into()],
                    rodovia: None,
                    extensao_km: Some(1.0),
                    geometry: Some(vec![[1.0, 2.0], [3.0, 4.0]]),
                    conec: vec!["b".into()],
                },
            ),
            (
                "b".to_string(),
                No {
                    mun: vec!["Feira de Santana".into()],
                    rodovia: None,
                    extensao_km: Some(1.0),
                    geometry: Some(vec![[3.0, 4.0], [5.0, 6.0]]),
                    conec: vec!["a".into()],
                },
            ),
        ]);
        let grafo = Grafo::novo(nos);
        let rota = grafo.rota("Salvador", "Feira de Santana").expect("rota");
        // polyline única: emenda (3.0, 4.0) não se repete
        assert_eq!(rota.linha, vec![[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]);
        // marcador = 1º ponto do 1º segmento que contém o município
        assert_eq!(rota.marcadores.len(), 2);
        assert_eq!(rota.marcadores[0].nome, "Salvador");
        assert_eq!(rota.marcadores[0].coord, [1.0, 2.0]);
        assert_eq!(rota.marcadores[1].nome, "Feira de Santana");
        assert_eq!(rota.marcadores[1].coord, [3.0, 4.0]);
    }
}