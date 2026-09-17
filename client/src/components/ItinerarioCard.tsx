import { useState } from "react";
import { Itinerario, formatarBrl, formatarPartida, rotaNomes } from "../api";
import { ArrowDown, ArrowRight, RotaTraco } from "./icon";

type Props = {
  itinerario: Itinerario;
  reservando: boolean;
  onReservar: (itinerario: Itinerario) => void;
};

/** Uma opção de viagem: placa de itinerário com marcos, trechos e tarifa. */
export default function ItinerarioCard({ itinerario, reservando, onReservar }: Props) {
  const [confirmando, setConfirmando] = useState(false);
  const trechos = itinerario.trechos;
  const ultimo = trechos.length - 1;

  function reservar() {
    if (confirmando) {
      setConfirmando(false);
      onReservar(itinerario);
    } else {
      setConfirmando(true);
      setTimeout(() => setConfirmando(false), 4000);
    }
  }

  return (
    <article className="placa overflow-hidden">
      <div className="flex items-center justify-between gap-3 border-b border-edge px-4 py-3">
        <h3 className="letreiro min-w-0 text-[15px] leading-snug">
          <RotaTraco nomes={rotaNomes(trechos)} />
        </h3>
        <span className="shrink-0 rounded-md bg-accent px-2.5 py-1 text-sm font-extrabold tabular-nums text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.25)]">
          {formatarBrl(itinerario.total_centavos)}
        </span>
      </div>

      <ol className="px-4 py-3">
        {trechos.map((etapa, i) => (
          <li key={i}>
            <div className="flex gap-3">
              <div className="flex flex-col items-center">
                <span className="marco">{i + 1}</span>
                {i < ultimo && <span className="trilho my-1.5 min-h-7 flex-1" />}
              </div>
              <div className="min-w-0 pb-1.5">
                <p className="text-sm font-bold leading-snug">
                  {etapa.embarque} <ArrowRight className="h-3.5 w-3.5 text-muted" />{" "}
                  {etapa.desembarque}
                </p>
                <p className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-muted">
                  <span className="font-bold tabular-nums text-ink">
                    {formatarPartida(etapa.partida)}
                  </span>
                  <span>· carona #{etapa.carona_id}</span>
                  {etapa.assentos_disponiveis !== undefined && (
                    <span className={etapa.assentos_disponiveis === 0 ? "badge badge-cheio" : "badge"}>
                      {etapa.assentos_disponiveis === 0
                        ? "sem lugares"
                        : `${etapa.assentos_disponiveis} lugar(es)`}
                    </span>
                  )}
                </p>
              </div>
            </div>

            {i < ultimo && (
              <div className="mb-1.5 ml-[31px] mt-1 flex items-center gap-2">
                <ArrowDown className="h-3.5 w-3.5 text-accent-ink" />
                <span className="text-[10px] font-extrabold uppercase tracking-[0.16em] text-muted">
                  baldeação
                </span>
                <span className="h-px flex-1 bg-edge" />
              </div>
            )}
          </li>
        ))}
      </ol>

      <footer className="flex items-center justify-between gap-3 border-t border-edge bg-surface2 px-4 py-3">
        <span className="text-xs text-muted">
          {trechos.length > 1
            ? `${trechos.length - 1} baldeação(ões) · acompanhe o embarque em cada marco`
            : "viagem direta, sem baldeação"}
        </span>
        <button className="btn btn-primary shrink-0" onClick={reservar} disabled={reservando}>
          {reservando
            ? "reservando…"
            : confirmando
              ? "confirmar reserva?"
              : `reservar · ${formatarBrl(itinerario.total_centavos)}`}
        </button>
      </footer>
    </article>
  );
}