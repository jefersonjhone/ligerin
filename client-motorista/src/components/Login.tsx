import { useState } from "react";
import { lpc, parsePayload } from "../api";
import { LogoMark } from "./icon";

type Props = {
  profiletype: "passageiro" | "motorista";
  onEntrar: (token: string, usuario: string, id: number) => void;
};

export default function Login({ profiletype, onEntrar }: Props) {
  const [modo, setModo] = useState<"entrar" | "registrar">("entrar");
  const [usuario, setUsuario] = useState("");
  const [senha, setSenha] = useState("");
  const [ocupado, setOcupado] = useState(false);
  const [error, setError] = useState("");

  async function entrar() {
    try {
      setError("");
      setOcupado(true);
      const data = await lpc(null, "POST", ".login", { nome: usuario.trim(), senha });
      const payload = parsePayload<{ token?: string; id?: number }>(data);
      const token = payload?.token ?? "";
      const id = payload?.id ?? 0;
      if (!token) throw new Error("resposta sem token");
      onEntrar(token, usuario.trim(), id);
    } catch (err) {
      setError(String(err));
      setOcupado(false);
    }
  }

  async function registrar() {
    try {
      setError("");
      setOcupado(true);
      await lpc(null, "POST", ".users", { nome: usuario.trim(), senha, profiletype });
      await entrar();
    } catch (err) {
      setError(String(err));
      setOcupado(false);
    }
  }

  return (
    <main className="mx-auto mt-[8vh] max-w-sm px-5">
      <div className="mb-7 flex flex-col items-center gap-3 text-center">
        <LogoMark className="h-12 w-12" />
        <h1 className="letreiro text-3xl">Ligerin</h1>
        <span className="rounded-md bg-accent px-2 py-1 text-[11px] font-extrabold uppercase tracking-[0.16em] text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.25)]">
          {profiletype}
        </span>
      </div>

      <div className="placa p-5 sm:p-6">
        <div className="flex flex-col gap-3.5">
          <label className="flex flex-col gap-1.5">
            <span className="campo-label">usuário</span>
            <input
              className="input"
              value={usuario}
              onChange={(e) => setUsuario(e.currentTarget.value)}
              placeholder="nome de usuário"
              autoComplete="username"
            />
          </label>
          <label className="flex flex-col gap-1.5">
            <span className="campo-label">senha</span>
            <input
              className="input"
              value={senha}
              onChange={(e) => setSenha(e.currentTarget.value)}
              placeholder="••••••••"
              type="password"
              autoComplete={modo === "entrar" ? "current-password" : "new-password"}
              onKeyDown={(e) => e.key === "Enter" && (modo === "entrar" ? entrar() : registrar())}
            />
          </label>
          <button className="btn btn-primary w-full" onClick={entrar} disabled={ocupado}>
            {ocupado ? "entrando…" : "entrar"}
          </button>
        </div>

        <div className="mt-4 flex justify-center">
          <button
            className="btn btn-link"
            onClick={() => setModo(modo === "entrar" ? "registrar" : "entrar")}
          >
            {modo === "entrar" ? "não tem conta? registrar" : "já tem conta? entrar"}
          </button>
        </div>

        {error && <p className="erro mt-4">{error}</p>}
      </div>
    </main>
  );
}