import { Fragment } from "react";

/** Símbolos do mundo "sinalização rodoviária BR" — traço 2, pontas redondas. */

type Props = { className?: string };

export function RotaTraco({ nomes, className = "" }: { nomes: string[]; className?: string }) {
  return (
    <span className={`inline-flex flex-wrap items-center gap-x-1.5 gap-y-0.5 ${className}`}>
      {nomes.map((n, i) => (
        <Fragment key={i}>
          {i > 0 && <ArrowRight className="h-3.5 w-3.5 shrink-0 text-accent-ink" />}
          <span>{n}</span>
        </Fragment>
      ))}
    </span>
  );
}

export function LogoMark({ className = "h-6 w-6" }: Props) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <rect
        x="5"
        y="5"
        width="14"
        height="14"
        rx="4"
        transform="rotate(45 12 12)"
        fill="var(--accent)"
      />
      <path
        d="M9.6 15.4V8.6h5"
        stroke="#fff"
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );
}

export function ArrowRight({ className = "h-4 w-4" }: Props) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <path
        d="M4 12h15M13.5 6.5 19 12l-5.5 5.5"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );
}

export function ArrowDown({ className = "h-4 w-4" }: Props) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <path
        d="M12 4v15M6.5 13.5 12 19l5.5-5.5"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );
}

export function XMark({ className = "h-4 w-4" }: Props) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <path
        d="M6.5 6.5l11 11M17.5 6.5l-11 11"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        fill="none"
      />
    </svg>
  );
}

export function Atualizar({ className = "h-4 w-4" }: Props) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <path
        d="M19.5 11.5a7.5 7.5 0 1 0-1.7 5.4M19.9 5.6v5.5h-5.5"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );
}