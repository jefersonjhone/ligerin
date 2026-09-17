import { invoke } from "@tauri-apps/api/core";

export type LpcResponse = {
  status: number;
  resource: string;
  payload: string | null;
};
/** Segmento físico do grafo, devolvido pelo GET .rota. */
export type TrechoFisico = {
  id: string;
  mun: string[];
  rodovia: string | null;
  extensao_km: number | null;
};

/** Parada nomeada da rota: município + coordenada [lng, lat] no mapa. */
export type Marcador = {
  nome: string;
  coord: [number, number];
};

/** Polígono de um município, devolvido por .municipios.shapes*/

export type ShapeMunicipio =
  | { type: "Polygon"; coordinates: [number, number][][] }
  | { type: "MultiPolygon"; coordinates: [number, number][][][] };

/** Rota sugerida entre dois municípios (rascunho da carona). */
export type Rota = {
  origem: string;
  destino: string;
  distancia_km: number;
  municipios: string[];
  trechos: TrechoFisico[];
  /** Polyline única do percurso ([lng, lat]) para desenhar no mapa. */
  linha: [number, number][];
  /** Paradas (municípios) com coordenada de referência. */
  marcadores: Marcador[];
};

/** Trecho de uma carona: par de paradas com preço próprio, em centavos. */
export type TrechoCarona = {
  origem: string;
  destino: string;
  preco: number;
  km: number;
  assentos: number;
  ocupados: number;
};

export type Carona = {
  id: number;
  /** Id do usuário motorista (referência em vez do nome). */
  dono: number;
  /** Nome do dono, só para exibição. */
  dono_nome: string | null;
  partida: string; // "AAAA-MM-DDTHH:MM"
  trechos: TrechoCarona[];
};

/** Uma trecho de viagem de uma reserva (embarque/desembarque são paradas). */
export type TrechoViagem = {
  carona_id: number;
  embarque: string;
  desembarque: string;
  partida: string;
};

export type Reserva = {
  id: number;
  /** Id do usuário passageiro (referência em vez do nome). */
  dono: number;
  /** Nome do passageiro, só para exibição. */
  dono_nome: string | null;
  total_centavos: number;
  trechos: TrechoViagem[];
};


/** Chama o command genérico do Tauri (lpc_request). */
export async function lpc(
  token: string | null,
  method: string,
  resource: string,
  payload?: unknown,
): Promise<LpcResponse> {
  const response = await invoke<LpcResponse>("lpc_request", {
    method,
    resource,
    payload: payload ?? null,
    token: token || null,
  });

  // respostas de erro do protocolo chegam com status >= 400 — viram exceção
  if (response.status >= 400) {
    const body = parsePayload<{ error?: string }>(response);
    throw new Error(`(${response.status}) ${body?.error ?? response.resource}`);
  }

  return response;
}

/** Parse seguro do payload JSON da resposta. */
export function parsePayload<T>(response: LpcResponse): T | null {
  if (!response.payload) return null;
  try {
    return JSON.parse(response.payload) as T;
  } catch {
    return null;
  }
}


/** Nomes de todos os municípios conhecidos pelo grafo (para o datalist). */
export async function listarMunicipios(token: string): Promise<string[]> {
  const data = await lpc(token, "GET", ".municipios");
  return parsePayload<{ municipios: string[] }>(data)?.municipios ?? [];
}

/** Rota mais curta; `via` força passagem por municípios intermediários. */
export async function buscarRota(
  token: string,
  origem: string,
  destino: string,
  via?: string[],
): Promise<Rota> {
  const data = await lpc(token, "GET", ".rota", { origem, destino, via });
  const rota = parsePayload<Rota>(data);
  if (!rota) throw new Error("resposta de .rota sem payload");
  return rota;
}

/** Polígonos dos municípios (malha offline); sem lista, devolve todos. */
export async function listarShapes(
  token: string,
  municipios?: string[],
): Promise<Record<string, ShapeMunicipio>> {
  const data = await lpc(token, "GET", ".municipios.shapes", {
    municipios,
  });
  return parsePayload<{ shapes: Record<string, ShapeMunicipio> }>(data)?.shapes ?? {};
}

/** Publica uma carona: cadeia de trechos encadeados, cada um com preço e
 * assentos próprios (assentos por trecho permitem reembarque no caminho). */
export async function publicarCarona(
  token: string,
  partida: string,
  trechos: { origem: string; destino: string; preco: number; assentos: number }[],
): Promise<Carona> {
  const data = await lpc(token, "POST", ".caronas", { partida, trechos });
  const carona = parsePayload<{ carona: Carona }>(data)?.carona;
  if (!carona) throw new Error("resposta de .caronas sem carona");
  return carona;
}

export async function listarCaronas(token: string): Promise<Carona[]> {
  const data = await lpc(token, "GET", ".caronas");
  return parsePayload<{ caronas: Carona[] }>(data)?.caronas ?? [];
}

export async function cancelarCarona(token: string, id: number): Promise<void> {
  await lpc(token, "POST", `.caronas.${id}.cancelar`, {});
}

export async function listarReservas(token: string): Promise<Reserva[]> {
  const data = await lpc(token, "GET", ".reservas");
  return parsePayload<{ reservas: Reserva[] }>(data)?.reservas ?? [];
}

export async function cancelarReserva(token: string, id: number): Promise<void> {
  await lpc(token, "POST", `.reservas.${id}.cancelar`, {});
}

export function rotaDaReserva(reserva: Reserva): string {
  return rotaNomes(reserva.trechos).join(" → ");
}

/** Nomes dos municípios na ordem do percurso, sem repetir baldeações. */
export function rotaNomes(trechos: TrechoViagem[]): string[] {
  const nomes: string[] = [];
  for (const p of trechos) {
    if (nomes.length === 0 || nomes[nomes.length - 1] !== p.embarque) nomes.push(p.embarque);
    nomes.push(p.desembarque);
  }
  return nomes;
}


/** "12,50" | "12.50" | "R$ 12,50" → centavos; null se inválido. */
export function brlParaCentavos(texto: string): number | null {
  const limpo = texto.replace("R$", "").replace(/\s/g, "").replace(",", ".");
  if (!/^\d+(\.\d{1,2})?$/.test(limpo)) return null;
  return Math.round(parseFloat(limpo) * 100);
}

export function formatarBrl(centavos: number): string {
  return (centavos / 100).toLocaleString("pt-BR", {
    style: "currency",
    currency: "BRL",
  });
}

/** "2026-09-20T08:00" → "20/09/2026 · 08:00" (sem depender de fuso). */
export function formatarPartida(iso: string): string {
  const [data, hora] = iso.split("T");
  if (!data) return iso;
  const [a, m, d] = data.split("-");
  return `${d}/${m}/${a} · ${hora ?? ""}`;
}