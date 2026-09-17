import { useEffect, useState } from "react";
import {
  buscarItinerarios,
  cancelarReserva,
  formatarBrl,
  formatarPartida,
  Itinerario,
  listarMunicipios,
  listarReservas,
  reservar,
  Reserva,
  hoje,
} from "../api";
import { ArrowRight, Atualizar } from "./icon";
import ItinerarioCard from "./ItinerarioCard";
import ReservaCard from "./ReservaCard";

type Aba = "buscar" | "reservas";

const ABAS: { id: Aba; nome: string }[] = [
  { id: "buscar", nome: "Buscar viagem" },
  { id: "reservas", nome: "Minhas reservas" },
];

export default function PassageiroHome({
  token,
  usuarioId,
}: {
  token: string;
  usuarioId: number;
}) {
  const [aba, setAba] = useState<Aba>("buscar");
  const [municipios, setMunicipios] = useState<string[]>([]);
  const [origem, setOrigem] = useState("");
  const [destino, setDestino] = useState("");
  const [data, setData] = useState(hoje());
  const [opcoes, setOpcoes] = useState<Itinerario[] | null>(null);
  const [buscando, setBuscando] = useState(false);
  const [reservandoId, setReservandoId] = useState<number | null>(null);
  const [minhas, setMinhas] = useState<Reserva[]>([]);
  const [erro, setErro] = useState("");
  const [sucesso, setSucesso] = useState("");

  useEffect(() => {
    listarMunicipios(token)
      .then(setMunicipios)
      .catch(() => {
        /* server indisponível: inputs de texto continuam funcionando */
      });
  }, [token]);

  async function buscar() {
    setErro("");
    setSucesso("");
    const o = origem.trim();
    const d = destino.trim();
    if (!o || !d) return setErro("preencha origem e destino");
    if (o === d) return setErro("origem e destino devem ser diferentes");
    if (!data) return setErro("escolha o dia da viagem");
    setBuscando(true);
    try {
      const itinerarios = await buscarItinerarios(token, o, d, data);
      setOpcoes(itinerarios);
    } catch (err) {
      setErro(String(err));
      setOpcoes(null);
    } finally {
      setBuscando(false);
    }
  }

  async function carregarMinhas() {
    try {
      const reservas = await listarReservas(token);
      setMinhas(reservas.filter((r) => r.dono === usuarioId));
    } catch {
      // server offline: mantém a lista atual
    }
  }

  useEffect(() => {
    carregarMinhas();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, usuarioId]);

  async function reservarItinerario(itinerario: Itinerario) {
    setErro("");
    setSucesso("");
    setReservandoId(itinerario.total_centavos);
    try {
      const trechos = itinerario.trechos.map((p) => ({
        carona_id: p.carona_id,
        embarque: p.embarque,
        desembarque: p.desembarque,
      }));
      await reservar(token, trechos);
      setSucesso(
        `reserva confirmada por ${formatarBrl(itinerario.total_centavos)} — bom passeio!`,
      );
      setOpcoes(null);
      setOrigem("");
      setDestino("");
      await carregarMinhas();
      setAba("reservas");
    } catch (err) {
      setErro(String(err));
    } finally {
      setReservandoId(null);
    }
  }

  async function cancelar(id: number) {
    try {
      setErro("");
      await cancelarReserva(token, id);
      await carregarMinhas();
    } catch (err) {
      setErro(String(err));
    }
  }

  return (
    <main className="mx-auto max-w-4xl px-5 py-8">
      <nav
        className="mb-6 grid grid-cols-2 gap-1 rounded-xl border border-edge bg-surface p-1"
        aria-label="seções"
      >
        {ABAS.map(({ id, nome }) => {
          const contador = id === "reservas" ? minhas.length : 0;
          const ativa = aba === id;
          return (
            <button
              key={id}
              role="tab"
              aria-selected={ativa}
              className={`flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-sm font-bold transition duration-150 ${
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

      {aba === "buscar" && (
        <section className="placa p-5 sm:p-6">
          <h2 className="letreiro text-base">buscar viagem</h2>
          <p className="mb-5 mt-1.5 max-w-prose text-sm leading-relaxed text-muted">
            o servidor combina até 3 caronas (baldeações) entre origem e destino no dia
            escolhido, mostrando primeiro as opções com menos trechos.
          </p>

          <div className="grid grid-cols-1 items-end gap-3 md:grid-cols-[1fr_auto_1fr]">
            <label className="flex flex-col gap-1.5">
              <span className="campo-label">Origem</span>
              <input
                className="input"
                value={origem}
                onChange={(e) => setOrigem(e.currentTarget.value)}
                placeholder="cidade de partida"
                list="municipios"
              />
            </label>
            <div className="hidden h-10 items-center justify-center text-accent-ink md:flex">
              <ArrowRight className="h-5 w-5" />
            </div>
            <label className="flex flex-col gap-1.5">
              <span className="campo-label">Destino</span>
              <input
                className="input"
                value={destino}
                onChange={(e) => setDestino(e.currentTarget.value)}
                placeholder="cidade de chegada"
                list="municipios"
              />
            </label>
          </div>
          <datalist id="municipios">
            {municipios.map((m) => (
              <option key={m} value={m} />
            ))}
          </datalist>

          <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-[190px_1fr]">
            <label className="flex flex-col gap-1.5">
              <span className="campo-label">Dia da viagem</span>
              <input
                className="input"
                type="date"
                value={data}
                onChange={(e) => setData(e.currentTarget.value)}
                onBlur={(e) =>
                  e.currentTarget.value !== data && setData(e.currentTarget.value)
                }
              />
            </label>
            <div className="flex items-end">
              <button
                className="btn btn-primary w-full sm:w-auto sm:px-8"
                onClick={buscar}
                disabled={buscando}
              >
                {buscando ? "buscando…" : "buscar viagem"}
              </button>
            </div>
          </div>

          {opcoes !== null && (
            <div className="mt-6">
              {opcoes.length === 0 ? (
                <p className="rounded-lg border border-dashed border-edge2 px-4 py-3 text-sm text-muted">
                  nenhuma viagem de {origem || "?"} → {destino || "?"} em{" "}
                  {formatarPartida(`${data}T00:00`)} — tente outra data ou origens
                  próximas.
                </p>
              ) : (
                <div className="flex flex-col gap-4">
                  {opcoes.map((itinerario, i) => (
                    <ItinerarioCard
                      key={i}
                      itinerario={itinerario}
                      reservando={reservandoId === itinerario.total_centavos}
                      onReservar={reservarItinerario}
                    />
                  ))}
                </div>
              )}
            </div>
          )}
        </section>
      )}

      {aba === "reservas" && (
        <section className="placa p-5 sm:p-6">
          <div className="mb-4 flex items-center justify-between gap-3">
            <h2 className="letreiro text-base">minhas reservas</h2>
            <button className="btn btn-link" onClick={carregarMinhas}>
              <Atualizar className="h-4 w-4" />
              atualizar
            </button>
          </div>
          {minhas.length === 0 ? (
            <p className="rounded-lg border border-dashed border-edge2 px-4 py-3 text-sm text-muted">
              nenhuma reserva ainda — busque uma viagem na aba anterior; aqui aparecem as
              passagens emitidas.
            </p>
          ) : (
            <div className="flex flex-col gap-4">
              {minhas.map((reserva) => (
                <ReservaCard key={reserva.id} reserva={reserva} onCancelar={cancelar} />
              ))}
            </div>
          )}
        </section>
      )}

      {erro && <p className="erro mt-4">{erro}</p>}
      {sucesso && <p className="sucesso mt-4">{sucesso}</p>}
    </main>
  );
}