use super::*;

#[derive(Debug, Clone)]
pub struct AlchemyConfig {
    pub id: String,
    pub version: String,
    pub expected_genesis_hash: String,
    pub first_slot: u64,
    pub last_slot: u64,
}

#[derive(Debug, Clone)]
pub struct SourceLimits {
    pub max_requests: u64,
    pub max_account_reads: u64,
    pub max_download_bytes: u64,
    pub max_response_bytes: u64,
    pub deadline: Duration,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SourceCounters {
    pub requests: u64,
    /// Attempted JSON-RPC methods, including failed requests; not billed CUs.
    pub methods: std::collections::BTreeMap<String, u64>,
    pub account_reads: u64,
    pub downloaded_bytes: u64,
    /// With a durable budget, all full-response reservations. Otherwise completed
    /// response bytes plus conservative reservations for incomplete failures.
    pub charged_bytes: u64,
    pub failed_inspections: u64,
}

pub struct RpcHttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum TransportFailure {
    Timeout,
    ResponseTooLarge,
    Network,
    CredentialEcho,
}

/// Custom transports must honor both timeout and read bound; the adapter checks again.
/// Failures are typed so endpoint credentials cannot leak through exception strings.
pub trait RpcTransport: Send + Sync {
    fn post(
        &self,
        request: &[u8],
        timeout: Duration,
        max_response_bytes: u64,
    ) -> std::result::Result<RpcHttpResponse, TransportFailure>;
}

pub(in super::super) struct HttpsTransport {
    pub(in super::super) client: reqwest::blocking::Client,
    pub(in super::super) endpoint: String,
    pub(in super::super) credential: String,
}

impl HttpsTransport {
    pub(in super::super) fn from_environment() -> Result<Self> {
        let credential = std::env::var("API_ALCHEMY")
            .map_err(|_| SourceError::new("SOURCE_CONFIGURATION", "set API_ALCHEMY"))?;
        require(
            !credential.is_empty()
                && credential.len() <= 512
                && credential
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)),
            "SOURCE_CONFIGURATION",
            "invalid historical API credential shape",
        )?;
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .build()
            .map_err(|_| {
                SourceError::new("SOURCE_TRANSPORT", "HTTPS client initialization failed")
            })?;
        Ok(Self {
            client,
            endpoint: format!("https://solana-mainnet.g.alchemy.com/v2/{credential}"),
            credential,
        })
    }
}

impl RpcTransport for HttpsTransport {
    fn post(
        &self,
        request: &[u8],
        timeout: Duration,
        max_response_bytes: u64,
    ) -> std::result::Result<RpcHttpResponse, TransportFailure> {
        let response = self
            .client
            .post(&self.endpoint)
            .header("content-type", "application/json")
            .body(request.to_vec())
            .timeout(timeout)
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    TransportFailure::Timeout
                } else {
                    TransportFailure::Network
                }
            })?;
        let status = response.status().as_u16();
        if response
            .content_length()
            .is_some_and(|size| size > max_response_bytes)
        {
            return Err(TransportFailure::ResponseTooLarge);
        }
        let mut body = Vec::new();
        response
            .take(max_response_bytes + 1)
            .read_to_end(&mut body)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    TransportFailure::Timeout
                } else {
                    TransportFailure::Network
                }
            })?;
        if body.len() as u64 > max_response_bytes {
            return Err(TransportFailure::ResponseTooLarge);
        }
        if body
            .windows(self.credential.len())
            .any(|window| window == self.credential.as_bytes())
        {
            return Err(TransportFailure::CredentialEcho);
        }
        Ok(RpcHttpResponse { status, body })
    }
}
