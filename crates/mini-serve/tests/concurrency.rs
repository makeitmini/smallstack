use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use hyper::StatusCode;
use mini_serve::{empty, handler, RouteBuilder};

#[tokio::test]
async fn max_connections_limits_concurrent_requests() {
    let peak = Arc::new(AtomicUsize::new(0));
    let current = Arc::new(AtomicUsize::new(0));

    let port = {
        let peak = peak.clone();
        let current = current.clone();
        RouteBuilder::stateless()
            .with_max_connections(2)
            .get("/slow", handler(move |_req, _state| {
                let peak = peak.clone();
                let current = current.clone();
                async move {
                    let c = current.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(c, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    current.fetch_sub(1, Ordering::SeqCst);
                    Ok(empty(StatusCode::OK))
                }
            }))
            .seal()
            .bind_ephemeral()
            .await
            .unwrap()
    };

    let mut handles = Vec::new();
    for _ in 0..8 {
        let url = format!("http://127.0.0.1:{port}/slow");
        handles.push(tokio::spawn(async move {
            let resp = reqwest::get(&url).await.unwrap();
            assert_eq!(resp.status(), 200);
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let p = peak.load(Ordering::SeqCst);
    assert!(p <= 2, "peak concurrency was {p}, expected ≤ 2");
}
