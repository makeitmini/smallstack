#![cfg(feature = "tls")]

use std::sync::Arc;
use mini_serve::{RouteBuilder, handler, json};
use hyper::StatusCode;

#[tokio::test]
async fn https_get_returns_200_with_trusted_self_signed_cert() {
	// Arrange
	let _ = rustls::crypto::ring::default_provider().install_default();

	let subject_alt_names = vec!["localhost".to_string()];
	let cert = rcgen::generate_simple_self_signed(subject_alt_names).unwrap();
	let cert_der = cert.cert.der().clone();
	let key_der = cert.key_pair.serialize_der();

	let server_config = rustls::ServerConfig::builder()
		.with_no_client_auth()
		.with_single_cert(
			vec![rustls::pki_types::CertificateDer::from(cert_der.clone())],
			rustls::pki_types::PrivateKeyDer::Pkcs8(key_der.into()),
		)
		.unwrap();

	let app = RouteBuilder::stateless()
		.get("/health", handler(|_req, _state| async {
			Ok(json(StatusCode::OK, &serde_json::json!({"ok": true}))?)
		}))
		.seal();

	// Act
	let port = app.bind_tls_ephemeral(Arc::new(server_config)).await.unwrap();

	// Assert
	let client = reqwest::ClientBuilder::new()
		.add_root_certificate(
			reqwest::Certificate::from_der(&cert_der).unwrap()
		)
		.build()
		.unwrap();

	let resp = client
		.get(format!("https://localhost:{port}/health"))
		.send()
		.await
		.unwrap();

	assert_eq!(resp.status(), StatusCode::OK);
}
