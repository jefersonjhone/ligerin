//! Domínio de caronas compartilhadas.
//!
//! Uma carona é uma cadeia de trechos (`origem -> destino` consecutivos);
//! cada trecho tem seu próprio preço (definido pelo motorista) e contador de
//! assentos ocupados — o preço mora no próprio trecho.
//!
//! A reserva é **atômica**: fase de validação (todos os trechos cobertos com
//! assento livre) e só então o commit (incrementa todos). Tudo sob um único
//! `Mutex<CaronaStore>`, então entre validar e commitar nada muda.

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// Máximo de trechos por reserva (o mesmo limite da busca de itinerários).
pub const MAX_TRECHOS_VIAGEM: usize = 3;
/// Defesa contra payload gigante.
pub const MAX_TRECHOS_CARONA: usize = 100;
/// Limite de resultados da busca de itinerários.
const MAX_ITINERARIOS: usize = 20;

/// Um trecho da carona: percurso entre duas paradas consecutivas.
#[derive(Debug, Clone)]
pub struct Trecho {
    pub origem: String,
    pub destino: String,
    /// Preço do trecho em centavos, definido pelo motorista.
    pub preco: u32,
    /// Distância entre as paradas, calculada pelo servidor na publicação.
    pub km: f64,
    /// Vagas para passageiros neste trecho (definidas pelo motorista),
    /// permitindo reembarque após desembarques no meio do caminho.
    pub assentos: u32,
    /// Assentos ocupados neste trecho (invariante: ocupados <= assentos).
    pub ocupados: u32,
}

#[derive(Debug, Clone)]
pub struct Carona {
    pub id: u32,
    /// Id do usuário motorista (referência em vez do nome).
    pub dono: u32,
    /// Partida no formato "AAAA-MM-DDTHH:MM".
    pub partida: String,
    pub trechos: Vec<Trecho>,
}

/// Um trecho de viagem de uma reserva: uma carona + embarque/desembarque
/// (municípios-parada da carona). É a referência ao trecho da carona que o
/// passageiro vai percorrer.
#[derive(Debug, Clone)]
pub struct TrechoViagem {
    pub carona_id: u32,
    pub embarque: String,
    pub desembarque: String,
    /// Assentos livres no percurso coberto (mínimo entre os trechos da
    /// carona); preenchido pela busca de itinerários e 0 nas reservas.
    pub disponiveis: u32,
}

#[derive(Debug, Clone)]
pub struct Reserva {
    pub id: u32,
    /// Id do usuário passageiro (referência em vez do nome).
    pub dono: u32,
    pub trechos: Vec<TrechoViagem>,
    pub total_centavos: u32,
}

/// Itinerário: sequência de trechos entre origem e destino.
#[derive(Debug, Clone)]
pub struct Itinerario {
    pub trechos: Vec<TrechoViagem>,
    pub total_centavos: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Erro {
    CaronaInexistente,
    TrechoNaoAtendido,
    AssentoEsgotado,
    SemTrechos,
    TrechosDemais,
}

pub struct CaronaStore {
    caronas: HashMap<u32, Carona>,
    reservas: HashMap<u32, Reserva>,
    prox_carona: u32,
    prox_reserva: u32,
}

impl CaronaStore {
    pub fn new() -> Self {
        Self {
            caronas: HashMap::new(),
            reservas: HashMap::new(),
            prox_carona: 1,
            prox_reserva: 1,
        }
    }

    /// Publica uma carona já validada pelo handler (trechos encadeados,
    /// conectados no grafo, com `km` calculado). Cada trecho carrega preço e
    /// assentos próprios; `dono` é o id do motorista.
    pub fn publicar(
        &mut self,
        dono: u32,
        partida: &str,
        trechos: Vec<(String, String, u32, f64, u32)>,
    ) -> Carona {
        let id = self.prox_carona;
        self.prox_carona += 1;
        let trechos = trechos
            .into_iter()
            .map(|(origem, destino, preco, km, assentos)| Trecho {
                origem,
                destino,
                preco,
                km,
                assentos,
                ocupados: 0,
            })
            .collect();
        let carona =
            Carona { id, dono, partida: partida.to_string(), trechos };
        self.caronas.insert(id, carona.clone());
        carona
    }

    pub fn lista(&self) -> Vec<&Carona> {
        self.caronas.values().collect()
    }

    pub fn obter(&self, id: u32) -> Option<&Carona> {
        self.caronas.get(&id)
    }

    /// Partida de cada carona, para montar o JSON de trechos/reservas.
    pub fn partidas(&self) -> HashMap<u32, String> {
        self.caronas.iter().map(|(id, c)| (*id, c.partida.clone())).collect()
    }

    pub fn lista_reservas(&self) -> Vec<&Reserva> {
        self.reservas.values().collect()
    }

    /// Reserva atômica: valida todos os trechos de viagem e só então commita.
    ///
    /// Trechos de viagem sobrepostos na mesma carona usam o mesmo assento nos
    /// trechos em comum (são fundidos, e o preço é contado uma única vez).
    pub fn reservar(&mut self, dono: u32, trechos: Vec<TrechoViagem>) -> Result<Reserva, Erro> {
        if trechos.is_empty() {
            return Err(Erro::SemTrechos);
        }
        if trechos.len() > MAX_TRECHOS_VIAGEM {
            return Err(Erro::TrechosDemais);
        }

        // resolve os índices de trecho cobertos por cada
        // trecho de viagem.
        let mut por_carona: HashMap<u32, Vec<(usize, usize)>> = HashMap::new();
        for etapa in &trechos {
            let carona =
                self.caronas.get(&etapa.carona_id).ok_or(Erro::CaronaInexistente)?;
            let i = carona
                .trechos
                .iter()
                .position(|t| t.origem == etapa.embarque)
                .ok_or(Erro::TrechoNaoAtendido)?;
            let j = carona
                .trechos
                .iter()
                .position(|t| t.destino == etapa.desembarque)
                .ok_or(Erro::TrechoNaoAtendido)?;
            if i > j {
                return Err(Erro::TrechoNaoAtendido); // desembarque antes do embarque
            }
            por_carona.entry(etapa.carona_id).or_default().push((i, j));
        }

        let mut total_centavos = 0u32;
        let mut toca: Vec<(u32, usize)> = Vec::new();
        let mut caronas_ordenadas: Vec<u32> = por_carona.keys().copied().collect();
        caronas_ordenadas.sort_unstable();
        for carona_id in caronas_ordenadas {
            let carona = &self.caronas[&carona_id];
            let mut intervalos = por_carona.remove(&carona_id).expect("intervalos");
            intervalos.sort_unstable();

            // funde intervalos sobrepostos da mesma carona
            let mut fundidos: Vec<(usize, usize)> = Vec::new();
            for (i, j) in intervalos {
                if let Some((_, fim)) = fundidos.last_mut() {
                    if i <= *fim {
                        *fim = (*fim).max(j);
                        continue;
                    }
                }
                fundidos.push((i, j));
            }

            for (i, j) in fundidos {
                for t in i..=j {
                    let trecho = &carona.trechos[t];
                    if trecho.ocupados + 1 > trecho.assentos {
                        return Err(Erro::AssentoEsgotado);
                    }
                    total_centavos += trecho.preco;
                    toca.push((carona_id, t));
                }
            }
        }

        // sob o Mutex, os assentos validados ainda estão livres.
        for (carona_id, t) in &toca {
            if let Some(carona) = self.caronas.get_mut(carona_id) {
                carona.trechos[*t].ocupados += 1;
            }
        }

        let id = self.prox_reserva;
        self.prox_reserva += 1;
        let reserva = Reserva { id, dono, trechos, total_centavos };
        self.reservas.insert(id, reserva.clone());
        Ok(reserva)
    }

    /// Cancela uma reserva e libera os assentos dos trechos cobertos.
    pub fn cancelar_reserva(&mut self, id: u32) -> bool {
        let Some(reserva) = self.reservas.remove(&id) else {
            return false;
        };
        self.liberar(&reserva.trechos);
        true
    }

    /// Remove a carona e cancela as reservas que a usam — liberando os
    /// assentos que elas ocupavam nas outras caronas.
    pub fn remover_carona(&mut self, id: u32) -> bool {
        if !self.caronas.contains_key(&id) {
            return false;
        }
        let afetadas: Vec<u32> = self
            .reservas
            .iter()
            .filter(|(_, r)| r.trechos.iter().any(|p| p.carona_id == id))
            .map(|(rid, _)| *rid)
            .collect();
        for rid in afetadas {
            let trechos = self.reservas.get(&rid).expect("reserva existe").trechos.clone();
            self.liberar(&trechos);
            self.reservas.remove(&rid);
        }
        self.caronas.remove(&id);
        true
    }

    /// Decrementa as ocupações dos trechos ocupados .
    fn liberar(&mut self, trechos: &[TrechoViagem]) {
        for etapa in trechos {
            if let Some(carona) = self.caronas.get_mut(&etapa.carona_id) {
                if let (Some(i), Some(j)) = (
                    carona.trechos.iter().position(|t| t.origem == etapa.embarque),
                    carona.trechos.iter().position(|t| t.destino == etapa.desembarque),
                ) {
                    for t in i..=j {
                        carona.trechos[t].ocupados =
                            carona.trechos[t].ocupados.saturating_sub(1);
                    }
                }
            }
        }
    }

    /// Remove caronas cujos horários já passaram, junto com as reservas que
    pub fn podar_passadas(&mut self, hoje: &str) {
        let ids: Vec<u32> = self
            .caronas
            .iter()
            .filter(|(_, c)| c.partida.as_str() < hoje)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.remover_carona(id);
        }
    }

    /// Busca itinerários origem→destino no dia `data` (AAAA-MM-DD), com até
    /// `max_trechos` trechos. A partida não regride entre trechos e só aparecem
    /// caronas com assento livre em todos os trechos do percurso.
    pub fn itinerarios(
        &self,
        origem: &str,
        destino: &str,
        data: &str,
        max_trechos: usize,
    ) -> Vec<Itinerario> {
        let mut resultados = Vec::new();
        let mut etapas: Vec<TrechoViagem> = Vec::new();
        let mut visitadas: HashSet<u32> = HashSet::new();
        self.buscar(
            origem,
            destino,
            data,
            max_trechos,
            &mut etapas,
            0,
            &mut visitadas,
            None,
            &mut resultados,
        );
        // menos trechos primeiro, depois partida, depois preço
        resultados.sort_by(|a, b| {
            a.trechos
                .len()
                .cmp(&b.trechos.len())
                .then_with(|| {
                    partida_da(&self.caronas, &a.trechos).cmp(&partida_da(&self.caronas, &b.trechos))
                })
                .then_with(|| a.total_centavos.cmp(&b.total_centavos))
        });
        resultados.truncate(MAX_ITINERARIOS);
        resultados
    }

    #[allow(clippy::too_many_arguments)]
    fn buscar(
        &self,
        atual: &str,
        destino: &str,
        data: &str,
        max_trechos: usize,
        etapas: &mut Vec<TrechoViagem>,
        total: u32,
        visitadas: &mut HashSet<u32>,
        partida_anterior: Option<&str>,
        resultados: &mut Vec<Itinerario>,
    ) {
        if etapas.len() >= max_trechos {
            return;
        }
        for (carona_id, carona) in &self.caronas {
            if visitadas.contains(carona_id) {
                continue;
            }
            if !carona.partida.starts_with(data) {
                continue;
            }
            if let Some(ant) = partida_anterior {
                if carona.partida.as_str() <= ant {
                    continue; // partida não regride
                }
            }
            let trechos = &carona.trechos;
            for i in 0..trechos.len() {
                if trechos[i].origem != atual {
                    continue;
                }
                let mut soma = 0u32;
                let mut disponiveis = u32::MAX;
                for j in i..trechos.len() {
                    let t = &trechos[j];
                    // trecho cheio: percursos mais longos também passariam por ele
                    if t.ocupados + 1 > t.assentos {
                        break;
                    }
                    soma += t.preco;
                    // assentos livres = mínimo entre os trechos cobertos
                    disponiveis = disponiveis.min(t.assentos - t.ocupados);
                    if t.destino == destino {
                        let mut etapas_finais = etapas.clone();
                        etapas_finais.push(TrechoViagem {
                            carona_id: *carona_id,
                            embarque: atual.to_string(),
                            desembarque: destino.to_string(),
                            disponiveis,
                        });
                        resultados.push(Itinerario {
                            trechos: etapas_finais,
                            total_centavos: total + soma,
                        });
                        break; // paradas são únicas; o destino não reaparece
                    }
                    // trecho intermediário: desce em t.destino e troca de carona
                    let mut etapas2 = etapas.clone();
                    etapas2.push(TrechoViagem {
                        carona_id: *carona_id,
                        embarque: atual.to_string(),
                        desembarque: t.destino.clone(),
                        disponiveis,
                    });
                    visitadas.insert(*carona_id);
                    self.buscar(
                        &t.destino,
                        destino,
                        data,
                        max_trechos,
                        &mut etapas2,
                        total + soma,
                        visitadas,
                        Some(carona.partida.as_str()),
                        resultados,
                    );
                    visitadas.remove(carona_id);
                }
            }
        }
    }
}

fn partida_da(caronas: &HashMap<u32, Carona>, trechos: &[TrechoViagem]) -> String {
    trechos
        .first()
        .and_then(|p| caronas.get(&p.carona_id))
        .map(|c| c.partida.clone())
        .unwrap_or_default()
}


pub fn trecho_json(t: &Trecho) -> Value {
    json!({
        "origem": t.origem,
        "destino": t.destino,
        "preco": t.preco,
        "km": (t.km * 100.0).round() / 100.0,
        "assentos": t.assentos,
        "ocupados": t.ocupados,
    })
}

pub fn carona_json(c: &Carona, nome_dono: Option<&str>) -> Value {
    json!({
        "id": c.id,
        // referência pelo id; o nome vai só para exibição
        "dono": c.dono,
        "dono_nome": nome_dono,
        "partida": c.partida,
        "trechos": c.trechos.iter().map(trecho_json).collect::<Vec<_>>(),
    })
}

/// TrechoViagem com a partida da carona, para exibição em itinerários/reservas.
pub fn trecho_viagem_json(p: &TrechoViagem, partidas: &HashMap<u32, String>) -> Value {
    json!({
        "carona_id": p.carona_id,
        "embarque": p.embarque,
        "desembarque": p.desembarque,
        "partida": partidas.get(&p.carona_id),
    })
}

pub fn etapa_itinerario_json(p: &TrechoViagem, partidas: &HashMap<u32, String>) -> Value {
    json!({
        "carona_id": p.carona_id,
        "embarque": p.embarque,
        "desembarque": p.desembarque,
        "partida": partidas.get(&p.carona_id),
        "assentos_disponiveis": p.disponiveis,
    })
}

pub fn reserva_json(r: &Reserva, partidas: &HashMap<u32, String>, nome_dono: Option<&str>) -> Value {
    json!({
        "id": r.id,
        // referência pelo id; o nome vai só para exibição
        "dono": r.dono,
        "dono_nome": nome_dono,
        "total_centavos": r.total_centavos,
        "trechos": r.trechos.iter().map(|p| trecho_viagem_json(p, partidas)).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_com_uma_carona(assentos: u32) -> (CaronaStore, u32) {
        let mut store = CaronaStore::new();
        let carona = store.publicar(
             1,
            "2026-09-20T08:00",
            vec![
                ("Salvador".into(), "Santo Amaro".into(), 1000, 2.0, assentos),
                ("Santo Amaro".into(), "Feira de Santana".into(), 2000, 2.0, assentos),
            ],
        );
        (store, carona.id)
    }

    #[test]
    fn publicar_e_consultar() {
        let (store, id) = store_com_uma_carona(3);
        let carona = store.obter(id).expect("carona");
        assert_eq!(carona.trechos.len(), 2);
        assert_eq!(carona.trechos[0].preco, 1000);
        assert_eq!(carona.trechos[0].assentos, 3);
        assert_eq!(carona.trechos[0].ocupados, 0);
    }

    #[test]
    fn assentos_podem_variar_por_trecho() {
        // reembarque no meio do caminho: mais assentos no trecho seguinte
        let mut store = CaronaStore::new();
        let carona = store.publicar(
             1,
            "2026-09-20T08:00",
            vec![
                ("Salvador".into(), "Santo Amaro".into(), 1000, 2.0, 2),
                ("Santo Amaro".into(), "Feira de Santana".into(), 2000, 2.0, 4),
            ],
        );
        assert_eq!(carona.trechos[0].assentos, 2);
        assert_eq!(carona.trechos[1].assentos, 4);
    }

    #[test]
    fn reserva_cobre_trechos_e_conta_preco() {
        let (mut store, id) = store_com_uma_carona(2);
        let reserva = store
            .reservar(
                 4,
                vec![TrechoViagem { carona_id: id, embarque: "Salvador".into(), desembarque: "Feira de Santana".into(), disponiveis: 0 }],
            )
            .expect("reserva");
        assert_eq!(reserva.total_centavos, 3000);
        let carona = store.obter(id).unwrap();
        assert_eq!(carona.trechos[0].ocupados, 1);
        assert_eq!(carona.trechos[1].ocupados, 1);
    }

    #[test]
    fn assento_esgotado_em_qualquer_trecho_roda_tudo() {
        let (mut store, id) = store_com_uma_carona(1);
        // ocupa o primeiro trecho com um trecho de viagem parcial
        store
            .reservar(
                 5,
                vec![TrechoViagem { carona_id: id, embarque: "Salvador".into(), desembarque: "Santo Amaro".into(), disponiveis: 0 }],
            )
            .expect("reserva parcial");
        // trecho de viagem completo não cabe (primeiro trecho cheio) e nada pode mudar
        let err = store
            .reservar(
                 6,
                vec![TrechoViagem { carona_id: id, embarque: "Salvador".into(), desembarque: "Feira de Santana".into(), disponiveis: 0 }],
            )
            .expect_err("deve falhar");
        assert_eq!(err, Erro::AssentoEsgotado);
        let carona = store.obter(id).unwrap();
        assert_eq!(carona.trechos[0].ocupados, 1);
        assert_eq!(carona.trechos[1].ocupados, 0, "commit parcial é proibido");
    }

    #[test]
    fn trechos_sobrepostas_na_mesma_carona_contam_um_assento() {
        let (mut store, id) = store_com_uma_carona(1);
        // Salvador->Santo Amaro + Santo Amaro->Feira = 1 assento na carona inteira
        store
            .reservar(
                 4,
                vec![
                    TrechoViagem { carona_id: id, embarque: "Salvador".into(), desembarque: "Santo Amaro".into(), disponiveis: 0 },
                    TrechoViagem { carona_id: id, embarque: "Santo Amaro".into(), desembarque: "Feira de Santana".into(), disponiveis: 0 },
                ],
            )
            .expect("reserva com trechos contíguas");
        let carona = store.obter(id).unwrap();
        assert_eq!(carona.trechos[0].ocupados, 1);
        assert_eq!(carona.trechos[1].ocupados, 1);
    }

    #[test]
    fn cancelar_reserva_libera_assentos() {
        let (mut store, id) = store_com_uma_carona(1);
        let reserva = store
            .reservar(
                 4,
                vec![TrechoViagem { carona_id: id, embarque: "Salvador".into(), desembarque: "Feira de Santana".into(), disponiveis: 0 }],
            )
            .expect("reserva");
        assert!(store.cancelar_reserva(reserva.id));
        let carona = store.obter(id).unwrap();
        assert_eq!(carona.trechos[0].ocupados, 0);
        assert_eq!(carona.trechos[1].ocupados, 0);
        assert!(!store.cancelar_reserva(999));
    }

    #[test]
    fn cancelar_carona_cancela_reservas_das_outras_caronas() {
        let mut store = CaronaStore::new();
        let a = store.publicar(
             1,
            "2026-09-20T08:00",
            vec![("Salvador".into(), "Santo Amaro".into(), 1000, 2.0, 2)],
        );
        let b = store.publicar(
             2,
            "2026-09-20T10:00",
            vec![("Santo Amaro".into(), "Feira de Santana".into(), 1500, 2.0, 2)],
        );
        let reserva = store
            .reservar(
                 4,
                vec![
                    TrechoViagem { carona_id: a.id, embarque: "Salvador".into(), desembarque: "Santo Amaro".into(), disponiveis: 0 },
                    TrechoViagem { carona_id: b.id, embarque: "Santo Amaro".into(), desembarque: "Feira de Santana".into(), disponiveis: 0 },
                ],
            )
            .expect("reserva multi-carona");
        assert_eq!(reserva.total_centavos, 2500);

        // cancelar a carona A derruba a reserva inteira e libera assentos na B
        assert!(store.remover_carona(a.id));
        let carona_b = store.obter(b.id).unwrap();
        assert_eq!(carona_b.trechos[0].ocupados, 0, "assento da B foi liberado");
        assert!(!store.remover_carona(999));
    }

    #[test]
    fn itinerarios_filtram_data_ordenam_e_limitam_trechos() {
        let mut store = CaronaStore::new();
        let a = store.publicar(
             1,
            "2026-09-20T08:00",
            vec![
                ("Salvador".into(), "Santo Amaro".into(), 1000, 2.0, 2),
                ("Santo Amaro".into(), "Feira de Santana".into(), 2000, 2.0, 2),
            ],
        );
        let b = store.publicar(
             2,
            "2026-09-20T10:00",
            vec![
                ("Feira de Santana".into(), "Jequié".into(), 1500, 2.0, 2),
                ("Jequié".into(), "Vitória da Conquista".into(), 2500, 2.0, 2),
            ],
        );
        // carona do dia seguinte não pode aparecer
        store.publicar(
             3,
            "2026-09-21T08:00",
            vec![("Salvador".into(), "Feira de Santana".into(), 5000, 4.0, 2)],
        );

        // rota direta (1 trecho de viagem) + rota com 2 trechos de viagem
        let its = store.itinerarios("Salvador", "Jequié", "2026-09-20", 3);
        assert_eq!(its.len(), 1);
        let it = &its[0];
        assert_eq!(it.trechos.len(), 2);
        assert_eq!(it.trechos[0].carona_id, a.id);
        assert_eq!(it.trechos[1].carona_id, b.id);
        assert_eq!(it.total_centavos, 1000 + 2000 + 1500);

        // mesmo dia, outro destino: 1 trecho de viagem
        let its = store.itinerarios("Salvador", "Feira de Santana", "2026-09-20", 3);
        assert_eq!(its.len(), 1);
        assert_eq!(its[0].trechos.len(), 1);
        assert_eq!(its[0].total_centavos, 3000);

        // dia errado: nada
        assert!(store.itinerarios("Salvador", "Jequié", "2026-09-22", 3).is_empty());
    }

    #[test]
    fn reserva_rejeita_trechos_fora_das_paradas() {
        let (mut store, id) = store_com_uma_carona(2);
        let err = store
            .reservar(
                 4,
                vec![TrechoViagem { carona_id: id, embarque: "Salvador".into(), desembarque: "Jequié".into(), disponiveis: 0 }],
            )
            .expect_err("Jequié não é parada");
        assert_eq!(err, Erro::TrechoNaoAtendido);

        let err = store
            .reservar(
                 4,
                vec![TrechoViagem { carona_id: 999, embarque: "Salvador".into(), desembarque: "Jequié".into(), disponiveis: 0 }],
            )
            .expect_err("carona inexistente");
        assert_eq!(err, Erro::CaronaInexistente);
    }
}