import { useEffect, useRef } from "react";
import * as L from "leaflet";
import "leaflet/dist/leaflet.css";
import type { Marcador, ShapeMunicipio } from "../api";

type Props = {
  /** Polyline única da rota em [lng, lat] (GeoJSON). */
  linha: [number, number][];
  /** Paradas nomeadas com coordenada [lng, lat]. */
  marcadores: Marcador[];
  /** Malha dos municípios envolvidos (sem tiles: só esses polígonos). */
  shapes: Record<string, ShapeMunicipio>;
  className?: string;
};

/** Converte [lng, lat] (GeoJSON) → [lat, lng] (Leaflet). */
const ll = (p: [number, number]): [number, number] => [p[1], p[0]];

/** Coordenada [lng, lat] utilizável (números finitos). */
const coordValida = (p: unknown): p is [number, number] =>
  Array.isArray(p) &&
  p.length === 2 &&
  Number.isFinite(p[0]) &&
  Number.isFinite(p[1]);

/** Anel de polígono utilizável (>= 3 pontos válidos). */
const anelValido = (anel: unknown): anel is [number, number][] =>
  Array.isArray(anel) && anel.length >= 3 && anel.every(coordValida);

/** Shape → Feature GeoJSON; `null` se o shape for inutilizável (defensivo). */
function shapeParaFeature(nome: string, shape: ShapeMunicipio): GeoJSON.Feature | null {
  if (shape.type === "Polygon") {
    if (!Array.isArray(shape.coordinates) || !shape.coordinates.every(anelValido)) return null;
    return { type: "Feature", properties: { nome }, geometry: shape };
  }
  if (shape.type === "MultiPolygon") {
    if (
      !Array.isArray(shape.coordinates) ||
      !shape.coordinates.every((p) => Array.isArray(p) && p.every(anelValido))
    ) {
      return null;
    }
    return { type: "Feature", properties: { nome }, geometry: shape };
  }
  return null;
}

/** Mapa Leaflet offline: malha dos municípios + polyline da rota + paradas. */
export default function MapaRota({ linha, marcadores, shapes, className = "" }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  // recria o mapa sempre que a rota/malha muda (dados pequenos; simples > incremental)
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let mapa: L.Map | null = null;
    try {
      const mapaNovo = L.map(el, {
        attributionControl: false,
        zoomControl: true,
        renderer: L.svg(),
      });
      mapa = mapaNovo;
      // setView antes de adicionar camadas: deixa o mapa "loaded" e o renderer
      // com _bounds definido — sem isso, Polygon._clipPoints lê undefined.min
      // e o Leaflet lança ao adicionar a malha (tela cinza no app).
      mapaNovo.setView([-12.5, -41.5], 6);

      const pontos: L.LatLngTuple[] = [];

      // malha dos municípios (fundo, sem tiles)
      const features: GeoJSON.Feature[] = Object.entries(shapes)
        .map(([nome, shape]) => shapeParaFeature(nome, shape))
        .filter((f): f is GeoJSON.Feature => f !== null);
      const malha = L.geoJSON(features, {
        style: {
          color: "var(--edge2)",
          weight: 1,
          fillColor: "var(--surface2)",
          fillOpacity: 0.6,
        },
      });
      malha.addTo(mapaNovo);
      malha.eachLayer((layer) => {
        try {
          const bounds = (layer as L.Polygon).getBounds();
          const sw = bounds.getSouthWest();
          const ne = bounds.getNorthEast();
          if (coordValida([sw.lng, sw.lat])) pontos.push([sw.lat, sw.lng]);
          if (coordValida([ne.lng, ne.lat])) pontos.push([ne.lat, ne.lng]);
        } catch {
          /* shape degenerado: fica fora do enquadre, sem derrubar o mapa */
        }
      });

      // polyline única da rota
      if (linha.length > 0) {
        const traco = linha.filter(coordValida).map(ll);
        if (traco.length > 0) {
          L.polyline(traco, {
            color: "var(--accent)",
            weight: 4,
            opacity: 0.9,
            lineJoin: "round",
            lineCap: "round",
          }).addTo(mapaNovo);
          pontos.push(...traco);
        }
      }

      // paradas: pontas maiores, intermediárias menores (sem L.marker default)
      marcadores.forEach((m, i) => {
        if (!coordValida(m.coord)) return;
        const pos = ll(m.coord);
        const ponta = i === 0 || i === marcadores.length - 1;
        pontos.push(pos);
        L.circleMarker(pos, {
          radius: ponta ? 7 : 5,
          color: "var(--surface)",
          weight: 2,
          fillColor: ponta ? "var(--accent)" : "var(--accent-ink)",
          fillOpacity: 1,
        })
          .bindTooltip(m.nome, { direction: "top", offset: [0, -6] })
          .addTo(mapaNovo);
      });

      if (pontos.length > 0) {
        mapaNovo.fitBounds(L.latLngBounds(pontos), { padding: [28, 28], maxZoom: 13 });
      }
    } catch (err) {
      // o mapa nunca pode derrubar o app: falha fica só no console
      console.error("mapa da rota falhou:", err);
    }
    return () => {
      try {
        mapa?.remove();
      } catch {
        /* já removido */
      }
    };
  }, [linha, marcadores, shapes]);

  return <div ref={ref} className={`mapa-rota ${className}`} aria-label="mapa da rota" />;
}