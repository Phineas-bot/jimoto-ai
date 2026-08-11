use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use hyper::Uri;

use crate::error::OllamaAdapterError;

pub(crate) const OLLAMA_HOST_ENV: &str = "OLLAMA_HOST";
const DEFAULT_PORT: u16 = 11_434;
const DEFAULT_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

/// A provider address that has passed GixGiz's loopback-only policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedEndpoint {
    address: SocketAddr,
    authority: String,
}

impl ValidatedEndpoint {
    /// Returns the documented default endpoint without DNS resolution.
    #[must_use]
    pub(crate) fn documented_default() -> Self {
        Self::from_address(SocketAddr::new(DEFAULT_HOST, DEFAULT_PORT))
    }

    /// Parses an `OLLAMA_HOST` value and rejects any non-loopback exposure.
    pub(crate) fn from_ollama_host(value: Option<&str>) -> Result<Self, OllamaAdapterError> {
        let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(Self::documented_default());
        };

        let normalized = if raw.contains("://") {
            raw.to_owned()
        } else {
            format!("http://{raw}")
        };
        let uri: Uri = normalized
            .parse()
            .map_err(|_| OllamaAdapterError::InvalidEndpoint)?;

        if uri.scheme_str() != Some("http") || uri.query().is_some() {
            return Err(OllamaAdapterError::InvalidEndpoint);
        }
        if !matches!(uri.path(), "" | "/") {
            return Err(OllamaAdapterError::InvalidEndpoint);
        }

        let authority = uri.authority().ok_or(OllamaAdapterError::InvalidEndpoint)?;
        if authority.as_str().contains('@') {
            return Err(OllamaAdapterError::InvalidEndpoint);
        }
        let explicit_port = if authority.as_str().starts_with('[') {
            let end = authority
                .as_str()
                .find(']')
                .ok_or(OllamaAdapterError::InvalidEndpoint)?;
            match &authority.as_str()[end + 1..] {
                "" => false,
                suffix if suffix.starts_with(':') => true,
                _ => return Err(OllamaAdapterError::InvalidEndpoint),
            }
        } else {
            authority.as_str().contains(':')
        };
        let port = match authority.port_u16() {
            Some(0) => return Err(OllamaAdapterError::InvalidEndpoint),
            Some(port) => port,
            None if explicit_port => return Err(OllamaAdapterError::InvalidEndpoint),
            None => DEFAULT_PORT,
        };
        let host = authority.host();
        let literal_host = host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(host);
        let ip = if host.eq_ignore_ascii_case("localhost") {
            DEFAULT_HOST
        } else {
            literal_host
                .parse::<IpAddr>()
                .map_err(|_| OllamaAdapterError::UnsafeEndpoint)?
        };

        if !ip.is_loopback() {
            return Err(OllamaAdapterError::UnsafeEndpoint);
        }

        Ok(Self::from_address(SocketAddr::new(ip, port)))
    }

    pub(crate) fn from_address(address: SocketAddr) -> Self {
        debug_assert!(address.ip().is_loopback());
        Self {
            authority: address.to_string(),
            address,
        }
    }

    #[must_use]
    pub(crate) const fn address(&self) -> SocketAddr {
        self.address
    }

    #[must_use]
    pub(crate) fn authority(&self) -> &str {
        &self.authority
    }

    #[must_use]
    pub(crate) fn child_environment_value(&self) -> String {
        self.authority.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_documented_ipv4_loopback_endpoint() {
        let endpoint = ValidatedEndpoint::from_ollama_host(None).expect("default is valid");

        assert_eq!(endpoint.address(), "127.0.0.1:11434".parse().unwrap());
    }

    #[test]
    fn pins_localhost_and_accepts_literal_loopback_only() {
        let localhost = ValidatedEndpoint::from_ollama_host(Some("localhost:22434"))
            .expect("localhost is pinned");
        let ipv6 = ValidatedEndpoint::from_ollama_host(Some("http://[::1]:22435"))
            .expect("IPv6 loopback is valid");

        assert_eq!(localhost.address(), "127.0.0.1:22434".parse().unwrap());
        assert_eq!(ipv6.address(), "[::1]:22435".parse().unwrap());
    }

    #[test]
    fn rejects_unspecified_remote_tls_and_path_endpoints() {
        for value in [
            "0.0.0.0:11434",
            "192.168.1.20:11434",
            "https://localhost:11434",
            "http://localhost:11434/api",
            "http://localhost:11434/?token=private",
            "http://localhost:0",
            "http://localhost:99999",
            "http://localhost:abc",
            "http://user@localhost:11434",
            "http://[::1]garbage",
        ] {
            assert!(
                ValidatedEndpoint::from_ollama_host(Some(value)).is_err(),
                "{value}"
            );
        }
    }
}
