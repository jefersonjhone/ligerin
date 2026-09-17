import { LogoMark } from "./icon";

type Props = { usuario: string; onSair: () => void };

export default function Header({ usuario, onSair }: Props) {
  return (
    <header className="sticky top-0 z-10 border-b border-edge bg-bg/90 backdrop-blur">
      <div className="mx-auto flex max-w-4xl items-center justify-between gap-4 px-5 py-3">
        <div className="flex min-w-0 items-center gap-2.5">
          <LogoMark className="h-7 w-7 shrink-0" />
          <span className="letreiro text-lg">Ligerin</span>
          <span className="hidden shrink-0 rounded-md bg-accent px-1.5 py-0.5 text-[10px] font-extrabold uppercase tracking-[0.14em] text-white sm:inline">
            passageiro
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-3">
          <span className="max-w-40 truncate text-sm text-muted">olá, {usuario}</span>
          <button className="btn btn-link" onClick={onSair}>
            sair
          </button>
        </div>
      </div>
    </header>
  );
}