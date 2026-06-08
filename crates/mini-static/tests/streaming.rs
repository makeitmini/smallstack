use std::io::Write;

use mini_static::Server;

#[tokio::test]
async fn large_file_is_served_completely() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.bin");
    let expected_len = 4 * 1024 * 1024; // 4 MB
    let mut f = std::fs::File::create(&path).unwrap();
    let content = vec![0xABu8; expected_len];
    f.write_all(&content).unwrap();
    drop(f);

    let port = Server::new(dir.path())
        .run_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/big.bin"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), expected_len);
    assert!(body.iter().all(|&b| b == 0xAB));
}
