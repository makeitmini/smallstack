#![cfg(feature = "tls")]

use std::sync::Arc;
use crate::error::ServeError;

/// Load a TLS server configuration from environment variables.
///
/// Reads `SMALLSTACK_TLS_CERT` and `SMALLSTACK_TLS_KEY` environment variables,
/// which should point to PEM-format files on disk. Returns a configured
/// `ServerConfig` wrapped in `Arc` for thread-safe sharing.
///
/// # Errors
///
/// Returns `ServeError` if environment variables are missing, files cannot be read,
/// or PEM parsing fails.
pub fn load_server_config() -> Result<Arc<rustls::ServerConfig>, ServeError> {
	let cert_path = std::env::var("SMALLSTACK_TLS_CERT")
		.map_err(|_| ServeError::new(500, "SMALLSTACK_TLS_CERT not set"))?;
	let key_path = std::env::var("SMALLSTACK_TLS_KEY")
		.map_err(|_| ServeError::new(500, "SMALLSTACK_TLS_KEY not set"))?;

	let cert_pem = std::fs::read(&cert_path)
		.map_err(|e| ServeError::new(500, format!("failed to read cert: {e}")))?;
	let key_pem = std::fs::read(&key_path)
		.map_err(|e| ServeError::new(500, format!("failed to read key: {e}")))?;

	let certs = rustls_pemfile::certs(&mut &cert_pem[..])
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| ServeError::new(500, format!("failed to parse cert: {e}")))?;

	if certs.is_empty() {
		return Err(ServeError::new(500, "no certificates found in cert file"));
	}

	let key = rustls_pemfile::private_key(&mut &key_pem[..])
		.map_err(|e| ServeError::new(500, format!("failed to parse key: {e}")))?
		.ok_or_else(|| ServeError::new(500, "no private key found in key file"))?;

	let config = rustls::ServerConfig::builder()
		.with_no_client_auth()
		.with_single_cert(certs, key)
		.map_err(|e| ServeError::new(500, format!("failed to build config: {e}")))?;

	Ok(Arc::new(config))
}
