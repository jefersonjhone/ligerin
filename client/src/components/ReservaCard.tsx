import { useState } from "react";
import { formatarBrl, formatarPartida, Reserva, rotaNomes } from "../api";
import { RotaTraco } from "./icon";

type Props = {
  reserva: Reserva;
  onCancelar: (id: number) => void;
};

/** Uma reserva feita pelo passageiro: passagem perfurada com stub de tarifa. */
export default function ReservaCard({ reserva, onCancelar }: Props) {
  const [confirmando, setConfirmando] = useState(false);

  function cancelar() {
    if (confirmando) {
      setConfirmando(false);
      onCancelar(reserva.id);
    } else {
      setConfirmando(true);
      setTimeout(() => setConfirmando(false), 4000);
    }
  }

  return (
    <article className="passagem overflow-hidden">
      <div className="grid grid-cols-[1fr_auto]">
        <div className="min-w-0 px-4 py-3.5">
          <h3 className="letreiro text-[15px] leading-snug">
            <RotaTraco nomes={rotaNomes(reserva.trechos)} />
          </h3>
          <ol className="mt-2 flex flex-col gap-1.5">
            {reserva.trechos.map((etapa, i) => (
              <li key={i} className="flex flex-wrap items-baseline gap-x-2 text-sm">
                <span className="font-bold">
                  {etapa.embarque} → {etapa.desembarque}
                </span>
                <span className="text-xs tabular-nums text-muted">
                  {formatarPartida(etapa.partida)} · carona #{etapa.carona_id}
                  {i < reserva.trechos.length - 1 && " · baldeação"}
                </span>
              </li>
            ))}
          </ol>
        </div>

        <div className="flex flex-col items-center justify-center gap-0.5 border-l border-dashed border-edge2 bg-surface2 px-4 py-3 sm:px-6">
          <span className="campo-label">tarifa</span>
          <span className="text-base font-extrabold tabular-nums text-accent-ink">
            {formatarBrl(reserva.total_centavos)}
          </span>
        </div>
      </div>

      <div className="flex items-center justify-between gap-3 border-t border-edge bg-surface2 px-4 py-2.5">
        <span className="text-[11px] font-extrabold uppercase tracking-[0.14em] text-muted">
          nº {reserva.id}
        </span>
        <button className="btn btn-danger" onClick={cancelar}>
          {confirmando ? "confirmar cancelamento?" : "cancelar reserva"}
        </button>
      </div>
    </article>
  );
}