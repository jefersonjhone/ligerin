import { invoke } from "@tauri-apps/api/core";

export type LpcResponse = {
  status: number;
  resource: string;
  payload: string | null;
};


/** Um trecho de viagem: uma carona + embarque/desembarque (paradas). */
export type TrechoViagem = {
  carona_id: number;
  embarque: string;
  desembarque: string;
  partida: string; // "AAAA-MM-DDTHH:MM"
  /** Assentos livres no percurso coberto (só presente nos itinerários). */
  assentos_disponiveis?: number;
};

/** Opção de viagem: sequência de trechos entre origem e destino. */
export type Itinerario = {
  total_centavos: number;
  trechos: TrechoViagem[];
};

export type Reserva = {
  id: number;
  /** Id do passageiro (referência em vez do nome). */
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

  if (response.status >= 400) {
    const body = parsePayload<{ error?: string }>(response);
    throw new Error(`(${response.status}) ${body?.error ?? response.resource}`);
  }

  return response;
}

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

/** Busca opções de viagem no dia `data` (AAAA-MM-DD), com até 3 trechos. */
export async function buscarItinerarios(
  token: string,
  origem: string,
  destino: string,
  data: string,
): Promise<Itinerario[]> {
  const response = await lpc(token, "GET", ".itinerarios", { origem, destino, data });
  return parsePayload<{ itinerarios: Itinerario[] }>(response)?.itinerarios ?? [];
}

/** Reserva atômica: todas as trechos juntas ou nada (409 se esgotado). */
export async function reservar(
  token: string,
  trechos: { carona_id: number; embarque: string; desembarque: string }[],
): Promise<Reserva> {
  const response = await lpc(token, "POST", ".reservas", { trechos });
  const reserva = parsePayload<{ reserva: Reserva }>(response)?.reserva;
  if (!reserva) throw new Error("resposta de .reservas sem reserva");
  return reserva;
}

export async function listarReservas(token: string): Promise<Reserva[]> {
  const response = await lpc(token, "GET", ".reservas");
  return parsePayload<{ reservas: Reserva[] }>(response)?.reservas ?? [];
}

export async function cancelarReserva(token: string, id: number): Promise<void> {
  await lpc(token, "POST", `.reservas.${id}.cancelar`, {});
}


/** "Salvador → Santo Amaro → Feira" encadeando as trechos (baldeações
 * repetem o município, que é exibido uma única vez). */
export function rotaDeTrechos(trechos: TrechoViagem[]): string {
  return rotaNomes(trechos).join(" → ");
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

/** Data de hoje local no formato AAAA-MM-DD. */
export function hoje(): string {
  const d = new Date();
  const mes = String(d.getMonth() + 1).padStart(2, "0");
  const dia = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mes}-${dia}`;
}