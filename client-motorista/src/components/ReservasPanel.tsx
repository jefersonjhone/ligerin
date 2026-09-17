import { useState } from "react";
import { formatarBrl, formatarPartida, Reserva, rotaNomes } from "../api";
import { RotaTraco } from "./icon";

type Props = {
  reservas: Reserva[];
  onCancelar: (id: number) => void;
};

/** Manifesto de reservas nas caronas do motorista. */
export default function ReservasPanel({ reservas, onCancelar }: Props) {
  const [confirmando, setConfirmando] = useState<number | null>(null);

  function cancelar(id: number) {
    if (confirmando === id) {
      setConfirmando(null);
      onCancelar(id);
    } else {
      setConfirmando(id);
      setTimeout(() => setConfirmando((atual) => (atual === id ? null : atual)), 4000);
    }
  }

  if (reservas.length === 0) {
    return (
      <p className="rounded-lg border border-dashed border-edge2 px-4 py-3 text-sm text-muted">
        nenhuma reserva nas suas caronas ainda — quando um passageiro reservar, ela aparece
        aqui no manifesto.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {reservas.map((reserva) => (
        <article key={reserva.id} className="placa px-4 py-3.5">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h3 className="text-sm font-extrabold">
              {reserva.dono_nome ?? `passageiro #${reserva.dono}`}
              <span className="ml-2 font-bold text-muted">
                reserva #{reserva.id} · carona(s){" "}
                {[...new Set(reserva.trechos.map((p) => p.carona_id))].join(", ")}
              </span>
            </h3>
            <span className="text-sm font-extrabold tabular-nums text-accent-ink">
              {formatarBrl(reserva.total_centavos)}
            </span>
          </div>
          <p className="mt-1.5 text-[15px] font-semibold leading-snug">
            <RotaTraco nomes={rotaNomes(reserva.trechos)} />
          </p>
          <p className="mt-0.5 text-xs tabular-nums text-muted">
            {reserva.trechos.map((p, i) => (
              <span key={i}>
                {i > 0 && " · "}
                #{p.carona_id} {formatarPartida(p.partida)}
              </span>
            ))}
          </p>
          <div className="mt-2.5 flex justify-end border-t border-dashed border-edge pt-2.5">
            <button className="btn btn-danger" onClick={() => cancelar(reserva.id)}>
              {confirmando === reserva.id ? "confirmar cancelamento?" : "cancelar reserva"}
            </button>
          </div>
        </article>
      ))}
    </div>
  );
}