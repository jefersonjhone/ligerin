import { Fragment } from "react";
import type { Marcador, ShapeMunicipio } from "../api";
import MapaRota from "./MapaRota";
import { ArrowDown, ArrowRight, RotaTraco, XMark } from "./icon";

type Props = {
  paradas: string[];
  precos: string[];
  assentos: string[];
  distancia: number | null;
  /** Polyline única do percurso ([lng, lat]) para o mapa offline. */
  linha: [number, number][];
  /** Paradas nomeadas com coordenada, para o mapa offline. */
  marcadores: Marcador[];
  /** Polígonos dos municípios, para o mapa offline. */
  shapes: Record<string, ShapeMunicipio>;
  novaParada: string;
  ocupado: "" | "rota" | "publicar";
  data: string;
  hora: string;
  municipios: string[];
  onNovaParada: (v: string) => void;
  onAdicionarParada: () => void;
  onRemoverParada: (i: number) => void;
  onPreco: (i: number, v: string) => void;
  onAssentos: (i: number, v: string) => void;
  onData: (v: string) => void;
  onHora: (v: string) => void;
  onPublicar: () => void;
  onCancelarRascunho: () => void;
};

/** A "placa da rota": paradas como marcos numerados, trecho por trecho com
 * preço e vagas, data/hora da partida e ação de publicar. */
export default function PlacaRota(props: Props) {
  const {
    paradas,
    precos,
    assentos,
    distancia,
    linha,
    marcadores,
    shapes,
    novaParada,
    ocupado,
    data,
    hora,
    municipios,
    onNovaParada,
    onAdicionarParada,
    onRemoverParada,
    onPreco,
    onAssentos,
    onData,
    onHora,
    onPublicar,
    onCancelarRascunho,
  } = props;

  return (
    <div className="mt-5 border-t border-dashed border-edge pt-5">
      <div className="mb-4 flex flex-wrap items-baseline justify-between gap-2">
        <h3 className="letreiro text-[15px]">
          <RotaTraco nomes={[paradas[0], paradas[paradas.length - 1]]} />
        </h3>
        <span className="rounded-md border border-edge2 bg-surface2 px-2 py-0.5 text-xs font-bold tabular-nums text-muted">
          {distancia !== null ? `${distancia.toFixed(2).replace(".", ",")} km` : "rota"} ·{" "}
          {paradas.length} paradas
        </span>
      </div>

      {/* mapa offline: malha dos municípios + traçado da rota + paradas */}
      {(linha.length > 0 || marcadores.length > 0) && (
        <MapaRota
          linha={linha}
          marcadores={marcadores}
          shapes={shapes}
          className="h-56 sm:h-72"
        />
      )}

      {/* trilho contínuo: marcos numerados + editor de cada trecho */}
      <div className="ml-3 flex flex-col border-l-2 border-dashed border-edge2 pl-6">
        {paradas.map((parada, i) => (
          <Fragment key={`${parada}-${i}`}>
            <div className="relative flex items-center gap-2 py-1">
              <span
                className={`marco absolute left-[-31px] top-1/2 -translate-y-1/2 ${
                  i === 0 || i === paradas.length - 1 ? "marco-ponta" : ""
                }`}
              >
                {i + 1}
              </span>
              <span className="rounded-md border border-edge bg-surface px-2.5 py-1 text-sm font-bold shadow-sm">
                {parada}
              </span>
              {i > 0 && i < paradas.length - 1 && (
                <button
                  className="ml-1 inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs font-bold text-muted transition hover:bg-danger-soft hover:text-danger"
                  onClick={() => onRemoverParada(i)}
                  title={`remover parada ${parada}`}
                >
                  <XMark className="h-3.5 w-3.5" />
                  remover
                </button>
              )}
            </div>

            {i < paradas.length - 1 && (
              <div className="mb-1.5 flex flex-wrap items-center gap-x-3 gap-y-2 pl-2 pb-1.5">
                <ArrowDown className="h-3.5 w-3.5 text-accent-ink" />
                <span className="text-xs font-semibold text-muted">
                  {parada}{" "}
                  <ArrowRight className="h-3.5 w-3.5 text-accent-ink" />{" "}
                  {paradas[i + 1]}
                </span>
                <label className="flex items-center gap-1.5">
                  <span className="text-xs font-bold text-muted">R$</span>
                  <input
                    className="w-20 rounded-md border border-edge2 bg-surface px-2 py-1 text-right text-sm font-bold tabular-nums outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/20"
                    value={precos[i] ?? ""}
                    onChange={(e) => onPreco(i, e.currentTarget.value)}
                    placeholder="0,00"
                    inputMode="decimal"
                    aria-label={`preço do trecho ${parada} → ${paradas[i + 1]}`}
                  />
                </label>
                <label className="flex items-center gap-1.5">
                  <span
                    className="text-xs font-bold text-muted"
                    title="vagas para passageiros (o lugar do motorista não conta)"
                  >
                    vagas
                  </span>
                  <input
                    className="w-12 rounded-md border border-edge2 bg-surface px-2 py-1 text-right text-sm font-bold tabular-nums outline-none transition focus:border-accent focus:ring-2 focus:ring-accent/20"
                    value={assentos[i] ?? ""}
                    onChange={(e) => onAssentos(i, e.currentTarget.value)}
                    placeholder="3"
                    inputMode="numeric"
                    aria-label={`vagas do trecho ${parada} → ${paradas[i + 1]}`}
                  />
                </label>
              </div>
            )}
          </Fragment>
        ))}
      </div>

      <div className="mb-0.5 mt-4 flex gap-2.5">
        <input
          className="input flex-1"
          value={novaParada}
          onChange={(e) => onNovaParada(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && onAdicionarParada()}
          placeholder="parada extra (recalcula a rota)"
          list="municipios-rota"
        />
        <button className="btn btn-outline shrink-0" onClick={onAdicionarParada} disabled={ocupado === "rota"}>
          {ocupado === "rota" ? "recalculando…" : "adicionar parada"}
        </button>
      </div>
      <datalist id="municipios-rota">
        {municipios.map((m) => (
          <option key={m} value={m} />
        ))}
      </datalist>
      <p className="mb-5 mt-1.5 max-w-prose text-xs leading-relaxed text-muted">
        paradas intermediárias que você não quiser fazer podem ser removidas — o caminho
        físico não muda, só o trecho fica mais longo. As <strong>vagas</strong> são para
        passageiros (o lugar do motorista não conta) e permitem reembarque após
        desembarques no meio do caminho.
      </p>

      <div className="mb-4 grid grid-cols-1 gap-3 sm:grid-cols-2">
        <label className="flex flex-col gap-1.5">
          <span className="campo-label">Data da partida</span>
          <input
            className="input"
            type="date"
            value={data}
            onChange={(e) => onData(e.currentTarget.value)}
            onBlur={(e) => e.currentTarget.value !== data && onData(e.currentTarget.value)}
          />
        </label>
        <label className="flex flex-col gap-1.5">
          <span className="campo-label">Hora da partida</span>
          <input
            className="input"
            type="time"
            value={hora}
            onChange={(e) => onHora(e.currentTarget.value)}
            onBlur={(e) => e.currentTarget.value !== hora && onHora(e.currentTarget.value)}
          />
        </label>
      </div>

      <div className="flex items-center justify-between gap-3 border-t border-edge pt-4">
        <button className="btn btn-link" onClick={onCancelarRascunho} disabled={ocupado !== ""}>
          cancelar rascunho
        </button>
        <button className="btn btn-primary px-6" onClick={onPublicar} disabled={ocupado !== ""}>
          {ocupado === "publicar" ? "publicando…" : "publicar carona"}
        </button>
      </div>
    </div>
  );
}