//! The server's requests to other hosts: screens' data sources and images, and
//! TRMNL's firmware. Reads are bounded, and failures are described without their
//! URLs, whose path or query may carry a secret.

use std::time::Duration;

/// An HTTP client and the time each of its requests may take.
#[derive(Clone)]
pub struct Outbound {
    client: reqwest::Client,
    timeout: Duration,
}

/// A client whose HTTPS trusts the host's certificate store, so a host behind a
/// private CA works where the host trusts it; every client of the server is
/// built here. Loading that store is the only part that can fail, as on a bare
/// container without one; such a host still serves devices, and its HTTPS
/// requests fail with a certificate error until it has a store and the server
/// restarts.
///
/// # Panics
///
/// When even a client that trusts no certificate cannot be built.
pub fn client(
    timeout: Duration,
    configure: fn(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
) -> reqwest::Client {
    let builder = || {
        configure(
            reqwest::Client::builder()
                .timeout(timeout)
                .user_agent(concat!("afterimage/", env!("CARGO_PKG_VERSION"))),
        )
    };
    builder().build().unwrap_or_else(|error| {
        tracing::warn!(
            %error,
            "HTTPS requests will fail: the host's CA certificates could not be loaded; install them and restart"
        );
        builder()
            .tls_certs_only([])
            .build()
            .expect("a client that trusts no certificate needs no store")
    })
}

impl Outbound {
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            client: client(timeout, std::convert::identity),
            timeout,
        }
    }

    #[must_use]
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Sends the request and reads at most `limit` bytes of a successful answer.
    ///
    /// # Errors
    ///
    /// The failure in words: no answer, an HTTP error status, or more than
    /// `limit` bytes.
    pub async fn read(
        &self,
        request: reqwest::RequestBuilder,
        limit: usize,
    ) -> Result<Vec<u8>, String> {
        let mut response = request
            .send()
            .await
            .map_err(|error| self.describe(&error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("answered HTTP {status}"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| self.describe(&error))?
        {
            if bytes.len() + chunk.len() > limit {
                return Err(format!("answered more than {limit} bytes"));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    /// A failed request in words, without its URL: reqwest's own message names
    /// the URL. The causes beneath it (the connection, the name lookup, TLS) do
    /// not.
    fn describe(&self, error: &reqwest::Error) -> String {
        if error.is_timeout() {
            return format!("gave no answer within {} seconds", self.timeout.as_secs());
        }
        let mut message = if error.is_connect() {
            "could not connect".to_owned()
        } else if error.is_body() || error.is_decode() {
            "its answer broke off".to_owned()
        } else {
            "the request failed".to_owned()
        };
        let mut cause = std::error::Error::source(error);
        while let Some(inner) = cause {
            message.push_str(": ");
            message.push_str(&inner.to_string());
            cause = inner.source();
        }
        message
    }
}
