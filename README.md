# Ligerin - VAIJUNTO - Caronas Compartilhadas

Plataforma de **caronas compartilhadas intermunicipais** para a Bahia: motoristas publicam viagens
com rota, data, preço e vagas **por trecho**; passageiros buscam origens → destino em um dia e
reservam itinerários compostos por **uma ou mais caronas encadeadas** (baldeações), com reserva
**atômica** e controle de concorrência sobre os assentos.

Trabalho da disciplina de Concorrência e Conectividade 2026.2,  implementado em Rust + Tauri/React,
com protocolo de aplicação próprio (**LPC**) sobre sockets TCP nativos, sem frameworks de RPC
ou mensageria.

## Arquitetura


- **Servidor** — thread por conexão (`Arc<LPC>` compartilhado), grafo rodoviário real do estado da Bahia com rodovias Federais, estaduais e municipais extraidos do portal do [denit](https://servicos.dnit.gov.br/vgeo/).
  itinerários por DFS limitada (até 3 caronas), reserva atômica sob `Mutex`
- **Cliente motorista** — publica carona com rota calculada e **mapa offline** (Leaflet sem
  tiles, malha dos municípios + polyline + paradas); acompanha ocupação/receita por trecho;
  cancela caronas.
- **Cliente passageiro** — busca itinerários no dia, confirma reserva atômica, consulta e
  cancela reservas.
- **Protocolo LPC** — crate `protocol/` compartilhado: enquadramento por `message_size`,
  metadados `message-id`/`authorization`, payload JSON UTF-8.


## Repositório

```
protocol/           crate do protocolo LPC ( formato de mensagem, parse/encode)
server/             servidor central (Rust) + testes automatizados
client/             cliente passageiro (Tauri 2 + React 19 + Tailwind 4)
client-motorista/   cliente motorista (idem; inclui mapa Leaflet offline)
data/               grafo rodoviário, shapes dos municípios e notebook usado para tratamento dos dados
docker-compose.yml  server + clientes em contêineres (UI via VNC)
```

## Como rodar

### Local (desenvolvimento)

```sh
# servidor (porta 8080; carrega data/ba_grafo.json + shapes)
cd server && cargo run

# clientes (cada um em seu terminal)
cd client && bun install && bun run tauri dev          # passageiro
cd client-motorista && bun install && bun run tauri dev # motorista
```

Login padrão: `admin` / `admin` (perfil motorista). Registre um usuário `passageiro`
para testar o outro lado.

### Docker 

```sh
docker compose up --build
```

> para essa etapa é necessário ter um client VNC intalado na máquina cliente
- `server` expõe a porta **8080** (LPC).
- `motorista` e `passageiro` são clientes Tauri headless com **Xvfb + x11vnc**: acesse as
  interfaces por VNC em `localhost:5900` (motorista) e `localhost:5901` (passageiro).
- Para distribuir em máquinas diferentes, rode apenas o serviço desejado em cada uma e aponte
  `LPC_SERVER_HOST` para o IP do servidor (variável de ambiente já suportada pelos clientes).

## Testes

```sh
cd server && cargo test            # 53 testes: protocolo, rotas, caronas, reservas, carga
cargo test carga_corrida_de_assentos_mede_metricas -- --nocapture   # corrida de assentos
```

A suíte inclui os cenários exigidos:

- **Disputa simultânea pelos mesmos trechos** — 8, 16 e 32 passageiros concorrem por 2 vagas:
  exatamente 2 reservas confirmam (201) e os demais recebem 409; **nenhum assento é vendido
  duas vezes** e nenhum itinerário é confirmado pela metade:

- **Atomicidade multi-carona** — 3 passageiros disputam itinerário de 2 caronas; só um
  confirma; os assentos ficam consistentes.
- **Confiabilidade** — cliente caindo no meio da mensagem não derruba o servidor nem deixa
  assento bloqueado; timeouts de leitura (5 s); caronas passadas são podadas.

## Documentação

A documentação será disponibilizada em breve com o relatório.

