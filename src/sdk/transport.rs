#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};

use superhuman_docs::{Error, Request, Response, Transport};

pub(super) struct Exchange {
    pub(super) expected_status: u16,
    pub(super) response: Response,
}

#[derive(Default)]
pub(super) struct TransportState {
    pub(super) exchange: Option<Exchange>,
}

pub(super) struct HttpTransport {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) state: Arc<Mutex<TransportState>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) agent: ureq::Agent,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) credential: String,
}

impl Transport for HttpTransport {
    fn send_request(&self, request: Request) -> Result<Response, Error> {
        send_http_request(self, request)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn send_http_request(transport: &HttpTransport, request: Request) -> Result<Response, Error> {
    let expected_status = request.expected_status;
    let http_request = transport
        .agent
        .request(request.method.as_str(), &request.url)
        .set("Authorization", &format!("Bearer {}", transport.credential))
        .set("Content-Type", "application/json");
    let http_response = match match request.body {
        Some(body) => http_request.send_bytes(&body),
        None => http_request.call(),
    } {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(error) => return Err(Error::transport(error)),
    };
    let status = http_response.status();
    let mut body = Vec::new();
    http_response
        .into_reader()
        .read_to_end(&mut body)
        .map_err(Error::transport)?;
    let response = Response { status, body };
    transport
        .state
        .lock()
        .map_err(|_| Error::transport("HTTP transport state lock poisoned"))?
        .exchange = Some(Exchange {
        expected_status,
        response: response.clone(),
    });
    Ok(response)
}

#[cfg(target_arch = "wasm32")]
fn send_http_request(_transport: &HttpTransport, request: Request) -> Result<Response, Error> {
    Err(Error::transport(format!(
        "{} is not available in DuckDB-Wasm builds",
        request.operation
    )))
}
