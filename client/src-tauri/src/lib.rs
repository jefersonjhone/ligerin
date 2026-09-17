// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod client;

use ligerin_protocol::Method;
use serde::Serialize;

/// Resposta devolvida ao frontend.
#[derive(Serialize)]
struct LpcResponse {
    status: u16,
    resource: String,
    payload: Option<String>,
}

/// Command único: GET ou POST para o server LPC.
#[tauri::command]
fn lpc_request(
    method: String,
    resource: String,
    payload: Option<serde_json::Value>,
    token: Option<String>,
) -> Result<LpcResponse, String> {
    // endereço do server LPC: env (container) ou local default
    let ip = std::env::var("LPC_SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("LPC_SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
    eprintln!("[ligerin] lpc_request -> {ip}:{port} {method} {resource}");
    let client = client::LPCClient { ip, port };

    let method = method
        .parse::<Method>()
        .map_err(|_| format!("método desconhecido: {method}"))?;
    let payload = payload.map(|value| value.to_string());

    match client.request(method, &resource, payload.as_deref(), token.as_deref()) {
        Ok(response) => Ok(LpcResponse {
            status: response.header.status.code(),
            resource: response.header.resource.name,
            payload: response.payload.and_then(|payload| payload.as_str().map(String::from)),
        }),
        Err(err) => {
            eprintln!("[ligerin] lpc_request -> falha: {err}");
            Err(err)
        }
    }
}

/// Loga as navegações do WebView (para diagnosticar páginas de erro do WebKit).
fn logar_navegacao(_webview: &tauri::Webview, payload: &tauri::webview::PageLoadPayload) {
    eprintln!("[ligerin] página carregada: {}", payload.url());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .on_page_load(logar_navegacao)
        .invoke_handler(tauri::generate_handler![lpc_request])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}