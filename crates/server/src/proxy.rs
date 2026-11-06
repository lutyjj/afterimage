//! What a reverse proxy in front of the server says about a request.

/// Whether the client reached the proxy over TLS, as its `X-Forwarded-Proto`
/// says; schemes are case-insensitive (RFC 3986).
#[must_use]
pub fn https(headers: &http::HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|scheme| scheme.to_str().ok())
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_forwarded_scheme_in_any_case() {
        let forwarded = |scheme: &str| {
            let mut headers = http::HeaderMap::new();
            headers.insert("x-forwarded-proto", scheme.parse().unwrap());
            https(&headers)
        };
        assert!(forwarded("https"));
        assert!(forwarded("HTTPS"));
        assert!(!forwarded("http"));
        assert!(!https(&http::HeaderMap::new()));
    }
}
