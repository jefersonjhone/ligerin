import { useEffect, useState } from "react";
import {
  brlParaCentavos,
  buscarRota,
  listarMunicipios,
  listarShapes,
  publicarCarona,
  type Rota,
  type ShapeMunicipio,
} from "../api";
import PlacaRota from "./PlacaRota";

type Props = {
  token: string;
  onPublicada: () => void;
};

const chave = (o: string, d: string) => `${o}→${d}`;

/** Soma dois preços em texto ("12,50") se ambos forem válidos; senão "". */
function somarPrecos(a: string, b: string): string {
  const ca = brlParaCentavos(a);
  const cb = brlParaCentavos(b);
  if (ca === null || cb === null) return "";
  return ((ca + cb) / 100).toFixed(2).replace(".", ",");
}

/** Maior de dois assentos em texto; "" se algum não for inteiro >= 1. */
function fundirAssentos(a: string, b: string): string {
  const na = Number.parseInt(a, 10);
  const nb = Number.parseInt(b, 10);
  if (!Number.isInteger(na) || !Number.isInteger(nb) || na < 1 || nb < 1) return "";
  return String(Math.max(na, nb));
}

export default function PublicarCarona({ token, onPublicada }: Props) {
  const [municipios, setMunicipios] = useState<string[]>([]);
  const [origem, setOrigem] = useState("");
  const [destino, setDestino] = useState("");
  const [paradas, setParadas] = useState<string[] | null>(null); // null = sem rascunho
  const [precos, setPrecos] = useState<string[]>([]); // um por trecho (paradas-1)
  const [assentosTrechos, setAssentosTrechos] = useState<string[]>([]);
  const [distancia, setDistancia] = useState<number | null>(null);
  const [rota, setRota] = useState<Rota | null>(null);
  const [shapes, setShapes] = useState<Record<string, ShapeMunicipio>>({});
  const [novaParada, setNovaParada] = useState("");
  const [dataPartida, setDataPartida] = useState("");
  const [horaPartida, setHoraPartida] = useState("");
  const [ocupado, setOcupado] = useState<"" | "rota" | "publicar">("");
  const [erro, setErro] = useState("");
  const [sucesso, setSucesso] = useState("");

  useEffect(() => {
    listarMunicipios(token)
      .then(setMunicipios)
      .catch(() => {
        /* server indisponível: inputs de texto continuam funcionando */
      });
  }, [token]);

  function avisar(msg: string) {
    setErro(msg);
    setSucesso("");
  }

  /** Passo 1: origem + destino → rota mais curta com os municípios atravessados. */
  async function calcularRota() {
    setErro("");
    setSucesso("");
    const o = origem.trim();
    const d = destino.trim();
    if (!o || !d) return avisar("preencha origem e destino");
    if (o === d) return avisar("origem e destino devem ser diferentes");
    setOcupado("rota");
    try {
      const rota = await buscarRota(token, o, d);
      const sequencia = deduplicar(rota.municipios);
      setParadas(sequencia);
      setPrecos(sequencia.slice(1).map(() => ""));
      setAssentosTrechos(sequencia.slice(1).map(() => ""));
      setDistancia(rota.distancia_km);
      setRota(rota);
      // malha do mapa (falha não impede o rascunho: sem polígonos, só a linha)
      try {
        const malha = await listarShapes(token, sequencia);
        setShapes(malha);
      } catch {
        setShapes({});
      }
    } catch (err) {
      avisar(String(err));
    } finally {
      setOcupado("");
    }
  }

  /** Passo 2: trocar parada ou recalcular a rota passando por outra cidade. */
  async function adicionarParada() {
    const nome = novaParada.trim();
    if (!nome || !paradas) return;
    if (paradas.includes(nome)) return avisar(`${nome} já é uma parada desta rota`);
    setOcupado("rota");
    try {
      const rota = await buscarRota(token, paradas[0], paradas[paradas.length - 1], [
        ...paradas,
        nome,
      ]);
      const sequencia = deduplicar(rota.municipios);
      // preserva preço e assentos dos trechos que continuam consecutivos
      const antigos = new Map(
        paradas.slice(0, -1).map((o, i) => [chave(o, paradas[i + 1]), i]),
      );
      const indices: (number | undefined)[] = sequencia
        .slice(0, -1)
        .map((o, i) => antigos.get(chave(o, sequencia[i + 1])));
      setParadas(sequencia);
      setPrecos(indices.map((i) => (i !== undefined ? precos[i] : "")));
      setAssentosTrechos(indices.map((i) => (i !== undefined ? assentosTrechos[i] : "")));
      setDistancia(rota.distancia_km);
      setRota(rota);
      // mescla a malha dos municípios (novos ou não), mantendo a que já temos
      try {
        const malha = await listarShapes(token, sequencia);
        setShapes((prev) => ({ ...prev, ...malha }));
      } catch {
        /* segue sem polígonos novos */
      }
      setNovaParada("");
      setErro("");
    } catch (err) {
      avisar(String(err));
    } finally {
      setOcupado("");
    }
  }

  /** Remove uma parada do meio; o trecho fundido herda soma dos preços e
   * o maior nº de assentos (o carro físico não mudou). */
  function removerParada(indice: number) {
    if (!paradas || indice === 0 || indice === paradas.length - 1) return;
    const novo = paradas.filter((_, i) => i !== indice);
    const antigos = new Map(
      paradas.slice(0, -1).map((o, i) => [chave(o, paradas[i + 1]), i]),
    );
    const idx = (o: string, d: string) => antigos.get(chave(o, d));
    const novoPrecos = novo
      .slice(0, -1)
      .map((o, i) => (idx(o, novo[i + 1]) !== undefined ? precos[idx(o, novo[i + 1])!] : ""));
    const novoAssentos = novo
      .slice(0, -1)
      .map((o, i) =>
        idx(o, novo[i + 1]) !== undefined ? assentosTrechos[idx(o, novo[i + 1])!] : "",
      );
    // o par (indice-1 → indice+1) é fusão
    const fusaoPreco = somarPrecos(precos[indice - 1], precos[indice]);
    if (fusaoPreco !== "") novoPrecos[indice - 1] = fusaoPreco;
    const fusaoAssentos = fundirAssentos(assentosTrechos[indice - 1], assentosTrechos[indice]);
    if (fusaoAssentos !== "") novoAssentos[indice - 1] = fusaoAssentos;
    setParadas(novo);
    setPrecos(novoPrecos);
    setAssentosTrechos(novoAssentos);
  }

  function definirPreco(i: number, valor: string) {
    const novo = [...precos];
    novo[i] = valor;
    setPrecos(novo);
  }

  function definirAssentos(i: number, valor: string) {
    const novo = [...assentosTrechos];
    novo[i] = valor;
    setAssentosTrechos(novo);
  }

  /** Passo 3: data/hora + preços + assentos por trecho → publica. */
  async function publicar() {
    if (!paradas) return;
    setErro("");
    setSucesso("");
    if (!dataPartida) return avisar("escolha a data da partida");
    if (!horaPartida) return avisar("escolha a hora da partida");
    const precosC = precos.map(brlParaCentavos);
    if (precosC.some((p) => p === null)) {
      return avisar("preencha o preço de todos os trechos (ex.: 12,50)");
    }
    const assentosC = assentosTrechos.map((a) => Number.parseInt(a, 10));
    if (assentosC.some((a) => !Number.isInteger(a) || a < 1)) {
      return avisar("informe a quantidade de assentos de cada trecho (mínimo 1)");
    }
    const trechos = paradas.slice(0, -1).map((o, i) => ({
      origem: o,
      destino: paradas[i + 1],
      preco: precosC[i]!,
      assentos: assentosC[i],
    }));

    setOcupado("publicar");
    try {
      await publicarCarona(token, `${dataPartida}T${horaPartida}`, trechos);
      setSucesso("carona publicada! Passageiros já podem procurar por ela.");
      setParadas(null);
      setPrecos([]);
      setAssentosTrechos([]);
      setDistancia(null);
      setRota(null);
      setShapes({});
      setOrigem("");
      setDestino("");
      setDataPartida("");
      setHoraPartida("");
      onPublicada();
    } catch (err) {
      avisar(String(err));
    } finally {
      setOcupado("");
    }
  }

  return (
    <div>
      {/* passo 1 — origem e destino */}
      <div className="grid grid-cols-1 items-end gap-3 md:grid-cols-2">
        <label className="flex flex-col gap-1.5">
          <span className="campo-label">Origem</span>
          <input
            className="input"
            value={origem}
            onChange={(e) => setOrigem(e.currentTarget.value)}
            placeholder="cidade de partida"
            list="municipios"
            disabled={!!paradas}
          />
        </label>
        <label className="flex flex-col gap-1.5">
          <span className="campo-label">Destino</span>
          <input
            className="input"
            value={destino}
            onChange={(e) => setDestino(e.currentTarget.value)}
            placeholder="cidade de chegada"
            list="municipios"
            disabled={!!paradas}
          />
        </label>
      </div>
      <datalist id="municipios">
        {municipios.map((m) => (
          <option key={m} value={m} />
        ))}
      </datalist>

      {!paradas && (
        <button className="btn btn-primary mt-3 px-6" onClick={calcularRota} disabled={ocupado !== ""}>
          {ocupado === "rota" ? "calculando rota…" : "calcular rota"}
        </button>
      )}

      {/* passo 2 e 3 — a placa da rota */}
      {paradas && (
        <PlacaRota
          paradas={paradas}
          precos={precos}
          assentos={assentosTrechos}
          distancia={distancia}
          linha={rota?.linha ?? []}
          marcadores={rota?.marcadores ?? []}
          shapes={shapes}
          novaParada={novaParada}
          ocupado={ocupado}
          data={dataPartida}
          hora={horaPartida}
          municipios={municipios}
          onNovaParada={setNovaParada}
          onAdicionarParada={adicionarParada}
          onRemoverParada={removerParada}
          onPreco={definirPreco}
          onAssentos={definirAssentos}
          onData={setDataPartida}
          onHora={setHoraPartida}
          onPublicar={publicar}
          onCancelarRascunho={() => setParadas(null)}
        />
      )}

      {erro && <p className="erro mt-3">{erro}</p>}
      {sucesso && <p className="sucesso mt-3">{sucesso}</p>}
    </div>
  );
}

/** Remove municípios repetidos mantendo a ordem (rota ida-e-volta de borda). */
function deduplicar(municipios: string[]): string[] {
  const vistos = new Set<string>();
  const unicos: string[] = [];
  for (const m of municipios) {
    if (!vistos.has(m)) {
      vistos.add(m);
      unicos.push(m);
    }
  }
  return unicos;
}