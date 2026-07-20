use std::sync::{Arc, Mutex};

use superhuman_docs::{Client, ClientOptions, Error, Response, DEFAULT_BASE_URL};

use crate::model::SuperhumanDocsClientConfig;

mod transport;

use transport::{HttpTransport, TransportState};

pub(crate) fn normalize_api_base(base: &str) -> String {
    base.trim_end_matches('/').to_string()
}

pub(crate) fn non_empty_string(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

pub(crate) struct SdkClient {
    client: Client,
    state: Arc<Mutex<TransportState>>,
    execution: Mutex<()>,
}

impl SdkClient {
    pub(crate) fn new(config: &SuperhumanDocsClientConfig) -> Result<Self, String> {
        let base_url = if config.endpoint.is_empty() {
            DEFAULT_BASE_URL
        } else {
            config.endpoint.as_str()
        };
        Self::at(base_url, &config.credential)
    }

    pub(crate) fn at(base_url: &str, _credential: &str) -> Result<Self, String> {
        let state = Arc::new(Mutex::new(TransportState::default()));
        let transport = HttpTransport {
            #[cfg(not(target_arch = "wasm32"))]
            state: Arc::clone(&state),
            #[cfg(not(target_arch = "wasm32"))]
            agent: ureq::AgentBuilder::new().build(),
            #[cfg(not(target_arch = "wasm32"))]
            credential: _credential.to_string(),
        };
        let options = ClientOptions::new(transport).with_base_url(normalize_api_base(base_url));
        let client = Client::new(options).map_err(|error| error.to_string())?;
        Ok(Self {
            client,
            state,
            execution: Mutex::new(()),
        })
    }

    pub(crate) fn execute<T>(
        &self,
        operation: impl FnOnce(&Client) -> Result<T, Error>,
    ) -> Result<String, String> {
        self.execute_inner(None, None, operation)?
            .ok_or_else(|| "SDK transport returned an unexpected accepted status".to_string())
    }

    pub(crate) fn execute_with_body<T>(
        &self,
        body: String,
        operation: impl FnOnce(&Client) -> Result<T, Error>,
    ) -> Result<String, String> {
        self.execute_inner(Some(body.into_bytes()), None, operation)?
            .ok_or_else(|| "SDK transport returned an unexpected accepted status".to_string())
    }

    pub(crate) fn execute_accepting_status<T>(
        &self,
        accepted_status: u16,
        operation: impl FnOnce(&Client) -> Result<T, Error>,
    ) -> Result<Option<String>, String> {
        self.execute_inner(None, Some(accepted_status), operation)
    }

    fn execute_inner<T>(
        &self,
        body_override: Option<Vec<u8>>,
        accepted_status: Option<u16>,
        operation: impl FnOnce(&Client) -> Result<T, Error>,
    ) -> Result<Option<String>, String> {
        let _execution = self
            .execution
            .lock()
            .map_err(|_| "SDK client execution lock poisoned".to_string())?;
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "HTTP transport state lock poisoned".to_string())?;
            state.exchange = None;
            state.body_override = body_override;
        }

        let result = operation(&self.client);
        let exchange = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "HTTP transport state lock poisoned".to_string())?;
            state.body_override = None;
            state.exchange.take()
        };

        match (result, exchange) {
            (Ok(_), Some(exchange)) => response_body(exchange.response).map(Some),
            (Err(Error::Deserialize { .. }), Some(exchange))
                if exchange.response.status == exchange.expected_status =>
            {
                response_body(exchange.response).map(Some)
            }
            (Err(Error::UnexpectedStatus { actual, .. }), Some(_))
                if accepted_status == Some(actual) =>
            {
                Ok(None)
            }
            (Err(error), _) => Err(error.to_string()),
            (Ok(_), None) => Err("SDK transport returned no response".to_string()),
        }
    }
}

fn response_body(response: Response) -> Result<String, String> {
    String::from_utf8(response.body).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn validate_token_at(base_url: &str, credential: &str) -> Result<(), String> {
    let sdk = SdkClient::at(base_url, credential)?;
    sdk.execute(|client| client.whoami(superhuman_docs::operations::WhoamiInput {}))?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn validate_token(credential: &str) -> Result<(), String> {
    validate_token_at(DEFAULT_BASE_URL, credential)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn validate_token(_credential: &str) -> Result<(), String> {
    Err("Whoami is not available in DuckDB-Wasm builds".to_string())
}
