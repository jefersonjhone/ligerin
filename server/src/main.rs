//! Servidor central do Ligerin: boot, aceitação de conexões e despacho LPC.
//!
//! A lógica está distribuída nos módulos irmãos:
//! - `store`: estado central (contas, sessões, grafo, shapes) e validações;
//! - `lpc`: roteamento do protocolo LPC;
//! - `handlers`: operações de cada recurso (auth, rotas, caronas, reservas);
//! - `carona`/`grafo`: domínio de caronas e grafo rodoviário.

mod carona;
mod grafo;
mod handlers;
mod lpc;
mod store;

#[cfg(test)]
mod tests;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use ligerin_protocol::{Request, RequestHeader, Status};

use crate::lpc::{error_response, LPC};

fn handle_client(mut stream: TcpStream, server: &LPC) {
    // ponto único de entrada: lê a start-line e o resto exato pelo message_size
    let mut leitor = BufReader::new(&stream);
    let mut start_line = String::new();
    match leitor.read_line(&mut start_line) {
        Ok(0) => {
            println!("conexão fechada pelo cliente");
            return;
        }
        Ok(_) => {}
        Err(err) => {
            eprintln!("erro ao ler a start-line: {err}");
            return;
        }
    }

    let Ok(header) = RequestHeader::parse(start_line.trim_end_matches('\n')) else {
        let _ = stream.write_all(
            &error_response(Status::BadRequest, ".erro", "start-line inválida").encode(),
        );
        return;
    };

    // framing: message_size é o tamanho TOTAL da mensagem
    let total = header.message_size as usize;
    if total < start_line.len() {
        let _ = stream.write_all(
            &error_response(
                Status::BadRequest,
                ".erro",
                "message_size menor que a start-line",
            )
            .encode(),
        );
        return;
    }

    let mut rest = vec![0u8; total - start_line.len()];
    if let Err(err) = leitor.read_exact(&mut rest) {
        // cliente caiu no meio do envio — a thread morre sem derrubar o server
        eprintln!("cliente desconectou no meio da mensagem: {err}");
        return;
    }

    let mut message = start_line;
    let Ok(body) = String::from_utf8(rest) else {
        let _ = stream.write_all(
            &error_response(Status::BadRequest, ".erro", "mensagem não é texto UTF-8").encode(),
        );
        return;
    };
    message.push_str(&body);

    let response = match Request::parse(&message) {
        Ok(request) => server.handle(&request),
        Err(err) => error_response(Status::BadRequest, ".erro", err.to_string()),
    };

    if let Err(err) = stream.write_all(&response.encode()) {
        eprintln!("erro ao escrever resposta: {err}");
    }

    println!("response enviada!");
}

/// Loop de aceitação: thread por conexão (== por request, no modelo atual).
fn serve(listener: TcpListener, server: std::sync::Arc<LPC>) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let server = std::sync::Arc::clone(&server);
                std::thread::spawn(move || {
                    if let Err(err) = stream.set_read_timeout(Some(Duration::from_secs(5))) {
                        println!("timeout não suportado: {err}");
                    }
                    handle_client(stream, &server);
                });
            }
            Err(err) => eprintln!("erro de conexão: {err}"),
        }
    }
}

fn main() {
    let lpc = LPC::new();
    println!(
        "grafo: {} segmentos, {} municípios",
        lpc.store.grafo.num_nos(),
        lpc.store.grafo.cidades().len()
    );

    let server = std::sync::Arc::new(lpc);

    let listener = match TcpListener::bind("0.0.0.0:8080") {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("erro ao iniciar o servidor: {err}");
            return;
        }
    };

    println!("listening at {}", listener.local_addr().unwrap());
    serve(listener, server);
}
