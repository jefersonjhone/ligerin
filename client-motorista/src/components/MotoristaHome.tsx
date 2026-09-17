import { useEffect, useState } from "react";
import {
  Carona,
  cancelarCarona,
  cancelarReserva,
  listarCaronas,
  listarReservas,
  Reserva,
} from "../api";
import { Atualizar } from "./icon";
import CaronaCard from "./CaronaCard";
import PublicarCarona from "./PublicarCarona";
import ReservasPanel from "./ReservasPanel";

type Aba = "publicar" | "caronas" | "reservas";

const ABAS: { id: Aba; nome: string }[] = [
  { id: "publicar", nome: "Publicar carona" },
  { id: "caronas", nome: "Minhas caronas" },
  { id: "reservas", nome: "Reservas" },
];

export default function MotoristaHome({
  token,
  usuarioId,
}: {
  token: string;
  usuarioId: number;
}) {
  const [aba, setAba] = useState<Aba>("publicar");
  const [minhas, setMinhas] = useState<Carona[]>([]);
  const [reservas, setReservas] = useState<Reserva[]>([]);
  const [erro, setErro] = useState("");

  async function carregar() {
    try {
      const [caronas, todasReservas] = await Promise.all([
        listarCaronas(token),
        listarReservas(token),
      ]);
      const minhasCaronas = caronas.filter((c) => c.dono === usuarioId);
      const ids = new Set(minhasCaronas.map((c) => c.id));
      setMinhas(minhasCaronas);
      setReservas(todasReservas.filter((r) => r.trechos.some((p) => ids.has(p.carona_id))));
    } catch {
      // server offline: mantém os dados atuais
    }
  }

  useEffect(() => {
    carregar();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, usuarioId]);

  async function cancelar(id: number) {
    try {
      setErro("");
      await cancelarCarona(token, id);
      await carregar();
    } catch (err) {
      setErro(String(err));
    }
  }

  async function recusarReserva(id: number) {
    try {
      setErro("");
      await cancelarReserva(token, id);
      await carregar();
    } catch (err) {
      setErro(String(err));
    }
  }

  // receita (em centavos) por carona, somando o total das reservas que a usam
  const receitaPorCarona = new Map<number, number>();
  for (const reserva of reservas) {
    for (const etapa of reserva.trechos) {
      receitaPorCarona.set(
        etapa.carona_id,
        (receitaPorCarona.get(etapa.carona_id) ?? 0) + reserva.total_centavos,
      );
    }
  }

  return (
    <main className="mx-auto max-w-4xl px-5 py-8">
      <nav
        className="mb-6 grid grid-cols-3 gap-1 rounded-xl border border-edge bg-surface p-1"
        aria-label="seções"
      >
        {ABAS.map(({ id, nome }) => {
          const contador =
            id === "caronas" ? minhas.length : id === "reservas" ? reservas.length : 0;
          const ativa = aba === id;
          return (
            <button
              key={id}
              role="tab"
              aria-selected={ativa}
              className={`flex items-center justify-center gap-2 rounded-lg px-2 py-2 text-[13px] font-bold transition duration-150 sm:px-3 sm:text-sm ${
                ativa
                  ? "bg-accent text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.22)]"
                  : "text-muted hover:bg-bg hover:text-ink"
              }`}
              onClick={() => setAba(id)}
            >
              {nome}
              {contador > 0 && (
                <span
                  className={`rounded-md px-1.5 py-0.5 text-[11px] font-extrabold tabular-nums ${
                    ativa ? "bg-black/20 text-white" : "bg-accent-soft text-accent-ink"
                  }`}
                >
                  {contador}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {aba === "publicar" && (
        <section className="placa p-5 sm:p-6">
          <h2 className="letreiro text-base">publicar carona</h2>
          <p className="mb-5 mt-1.5 max-w-prose text-sm leading-relaxed text-muted">
            informe origem e destino — o servidor calcula a rota mais curta e você define as
            paradas e o preço de cada trecho.
          </p>
          <PublicarCarona token={token} onPublicada={carregar} />
        </section>
      )}

      {aba === "caronas" && (
        <section className="placa p-5 sm:p-6">
          <div className="mb-4 flex items-center justify-between gap-3">
            <h2 className="letreiro text-base">minhas caronas</h2>
            <button className="btn btn-link" onClick={carregar}>
              <Atualizar className="h-4 w-4" />
              atualizar
            </button>
          </div>
          {minhas.length === 0 && (
            <p className="mb-4 rounded-lg border border-dashed border-edge2 px-4 py-3 text-sm text-muted">
              nenhuma carona publicada ainda — publique uma na aba anterior; cada carona
              aparece aqui com as vagas ocupadas por trecho.
            </p>
          )}
          <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-4">
            {minhas.map((carona) => (
              <CaronaCard
                key={carona.id}
                carona={carona}
                minha
                receita={receitaPorCarona.get(carona.id)}
                onCancelar={cancelar}
              />
            ))}
          </div>
        </section>
      )}

      {aba === "reservas" && (
        <section className="placa p-5 sm:p-6">
          <div className="mb-4 flex items-center justify-between gap-3">
            <h2 className="letreiro text-base">reservas nas minhas caronas</h2>
            <button className="btn btn-link" onClick={carregar}>
              <Atualizar className="h-4 w-4" />
              atualizar
            </button>
          </div>
          <ReservasPanel reservas={reservas} onCancelar={recusarReserva} />
        </section>
      )}

      {erro && <p className="erro mt-4">{erro}</p>}
    </main>
  );
}