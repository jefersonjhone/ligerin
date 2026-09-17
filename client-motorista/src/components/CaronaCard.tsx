import { useState } from "react";
import { Carona, formatarBrl, formatarPartida } from "../api";
import { ArrowDown } from "./icon";

type Props = {
  carona: Carona;
  minha?: boolean;
  receita?: number;
  onCancelar?: (id: number) => void;
};

export default function CaronaCard({ carona, minha, receita, onCancelar }: Props) {
  const [confirmando, setConfirmando] = useState(false);
  const cheia = carona.trechos.some((t) => t.ocupados >= t.assentos);
  const ultimo = carona.trechos.length - 1;

  function cancelar() {
    if (confirmando) {
      setConfirmando(false);
      onCancelar?.(carona.id);
    } else {
      setConfirmando(true);
      setTimeout(() => setConfirmando(false), 4000);
    }
  }

  return (
    <article className={`placa overflow-hidden ${cheia ? "border-danger/40" : ""}`}>
      <div className="flex items-baseline justify-between gap-2 border-b border-edge px-4 py-3">
        <h3 className="letreiro min-w-0 text-base tabular-nums">
          {formatarPartida(carona.partida)}
        </h3>
        <span className="shrink-0 text-xs font-bold text-muted">
          #{carona.id}
          {minha ? " · minha" : ` · ${carona.dono_nome ?? `dono #${carona.dono}`}`}
        </span>
      </div>

      <ul className="flex flex-col gap-1.5 px-4 py-3">
        {carona.trechos.map((t, i) => (
          <li
            key={i}
            className="flex flex-wrap items-center gap-x-2.5 gap-y-1 rounded-lg bg-surface2 px-2.5 py-2"
          >
            <span className="text-sm font-bold">{t.origem}</span>
            <ArrowDown className="h-3.5 w-3.5 rotate-[-90deg] text-muted" />
            <span className={`text-sm font-semibold ${i === ultimo ? "" : "text-muted"}`}>
              {t.destino}
            </span>
            <span className="ml-auto flex items-center gap-2.5">
              <span className="text-sm font-extrabold tabular-nums text-accent-ink">
                {formatarBrl(t.preco)}
              </span>
              <span className="text-xs tabular-nums text-muted">
                {t.km.toFixed(1).replace(".", ",")} km
              </span>
              {t.ocupados >= t.assentos ? (
                <span className="badge badge-cheio">cheio</span>
              ) : (
                <span className={`losango ${t.ocupados >= t.assentos ? "losango-cheio" : ""}`}>
                  <span>
                    {t.ocupados}/{t.assentos}
                  </span>
                </span>
              )}
            </span>
          </li>
        ))}
      </ul>

      <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-edge bg-surface2 px-4 py-3">
        <span className="text-xs text-muted">
          vagas por trecho:{" "}
          {carona.trechos.map((t) => `${t.ocupados}/${t.assentos}`).join(" · ")}
        </span>
        <span className="flex items-center gap-3">
          {minha && receita !== undefined && receita > 0 && (
            <span className="text-sm font-extrabold tabular-nums text-accent-ink">
              receita {formatarBrl(receita)}
            </span>
          )}
          {minha && onCancelar && (
            <button className="btn btn-danger" onClick={cancelar}>
              {confirmando ? "confirmar cancelamento?" : "cancelar carona"}
            </button>
          )}
        </span>
      </footer>
    </article>
  );
}