#![cfg(debug_assertions)]

use mini_static::{Broadcaster, ChangeType, ReloadEvent, Server};

#[tokio::test]
async fn livereload_endpoint_returns_event_stream_content_type() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html></html>").unwrap();

    let port = Server::new(dir.path())
        .run_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/__mini_reload"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "text/event-stream"
    );
}

#[tokio::test]
async fn html_response_contains_injected_script_in_debug() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html><body></body></html>").unwrap();

    let port = Server::new(dir.path())
        .run_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("EventSource('/__mini_reload')"),
        "expected injected script in HTML response, got: {body}"
    );
}

#[tokio::test]
async fn css_broadcast_delivers_css_type_event() {
    let broadcaster = Broadcaster::new();
    let mut rx = broadcaster.subscribe();

    broadcaster.broadcast(ReloadEvent {
        change_type: ChangeType::Css,
    });

    match rx.try_recv() {
        Ok(event) => assert!(
            matches!(event.change_type, ChangeType::Css),
            "expected Css, got {:?}",
            event.change_type
        ),
        Err(e) => panic!("expected event, got: {e:?}"),
    }
}

#[tokio::test]
async fn non_css_broadcast_delivers_other_type_event() {
    let broadcaster = Broadcaster::new();
    let mut rx = broadcaster.subscribe();

    broadcaster.broadcast(ReloadEvent {
        change_type: ChangeType::Html,
    });

    match rx.try_recv() {
        Ok(event) => assert!(
            matches!(event.change_type, ChangeType::Html),
            "expected Html, got {:?}",
            event.change_type
        ),
        Err(e) => panic!("expected event, got: {e:?}"),
    }
}

#[tokio::test]
async fn broadcaster_drops_disconnected_clients() {
    let broadcaster = Broadcaster::new();
    assert_eq!(broadcaster.sender_count(), 0);

    let rx = broadcaster.subscribe();
    assert_eq!(broadcaster.sender_count(), 1);

    drop(rx);
    broadcaster.broadcast(ReloadEvent {
        change_type: ChangeType::Other,
    });
    assert_eq!(broadcaster.sender_count(), 0);
}
