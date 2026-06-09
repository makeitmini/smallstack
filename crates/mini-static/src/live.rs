use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::SystemTime;

use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use http_body::{Body, Frame};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::error::StaticError;
use crate::handler::{Handler, RequestInfo, ResponseBody};

#[derive(Clone, Debug)]
pub struct ReloadEvent {
    pub change_type: ChangeType,
}

#[derive(Clone, Debug)]
pub enum ChangeType {
    Css,
    Script,
    Html,
    Other,
}

impl ChangeType {
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("css") => ChangeType::Css,
            Some("js" | "mjs") => ChangeType::Script,
            Some("html" | "htm") => ChangeType::Html,
            _ => ChangeType::Other,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeType::Css => "css",
            ChangeType::Script => "script",
            ChangeType::Html => "html",
            ChangeType::Other => "other",
        }
    }
}

pub struct Broadcaster {
    senders: Mutex<Vec<UnboundedSender<ReloadEvent>>>,
}

impl Broadcaster {
    pub fn new() -> Arc<Self> {
        Arc::new(Broadcaster {
            senders: Mutex::new(Vec::new()),
        })
    }

    pub fn broadcast(&self, event: ReloadEvent) {
        let mut senders = self.senders.lock().unwrap();
        senders.retain(|sender| sender.send(event.clone()).is_ok());
    }

    pub fn subscribe(&self) -> UnboundedReceiver<ReloadEvent> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.senders.lock().unwrap().push(tx);
        rx
    }

    pub fn sender_count(&self) -> usize {
        self.senders.lock().unwrap().len()
    }
}

pub(crate) struct SseHandler {
    pub broadcaster: Arc<Broadcaster>,
}

impl Handler for SseHandler {
    fn handle(
        &self,
        info: RequestInfo,
    ) -> Pin<Box<dyn std::future::Future<Output = Option<Response<ResponseBody>>> + Send + '_>>
    {
        if info.method != "GET" || info.path != "/__mini_reload" {
            return Box::pin(async { None });
        }
        let broadcaster = self.broadcaster.clone();
        Box::pin(async move {
            let rx = broadcaster.subscribe();
            let body = BoxBody::new(SseStream { rx });
            Some(
                Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/event-stream")
                    .header("cache-control", "no-cache")
                    .header("connection", "keep-alive")
                    .body(body)
                    .unwrap(),
            )
        })
    }
}

struct SseStream {
    rx: UnboundedReceiver<ReloadEvent>,
}

impl Body for SseStream {
    type Data = Bytes;
    type Error = StaticError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(event)) => {
                let data = serde_json::json!({"type": event.change_type.as_str()});
                let msg = format!(
                    "event: {}\ndata: {}\n\n",
                    event.change_type.as_str(),
                    data
                );
                Poll::Ready(Some(Ok(Frame::data(Bytes::from(msg)))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub(crate) fn inject_livereload_script(bytes: &[u8]) -> Option<Vec<u8>> {
    let content = std::str::from_utf8(bytes).ok()?;

    let insert_before = if let Some(pos) = content.find("</body>") {
        pos
    } else {
        content.find("</html>")?
    };

    let mut result = bytes.to_vec();
    result.splice(insert_before..insert_before, LIVERELOAD_SCRIPT.as_bytes().iter().copied());
    Some(result)
}

const LIVERELOAD_SCRIPT: &str = r#"<script>
(function(){var e=new EventSource('/__mini_reload');e.addEventListener('css',function(){var t=document.querySelectorAll('link[rel="stylesheet"]');for(var n=0;n<t.length;n++){var r=new URL(t[n].href);r.searchParams.set('_',Date.now()),t[n].href=r.toString()}});['html','script','other'].forEach(function(t){e.addEventListener(t,function(){location.reload()})})})();
</script>"#;

pub(crate) fn start_poller(dir: Arc<PathBuf>, broadcaster: Arc<Broadcaster>) {
    tokio::spawn(async move {
        let mut mtimes: HashMap<PathBuf, SystemTime> = HashMap::new();
        let mut first_pass = true;
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            let entries = walk_dir(dir.as_path()).await.unwrap_or_default();
            let mut current = HashMap::new();

            for path in entries {
                if let Ok(meta) = tokio::fs::metadata(&path).await {
                    if let Ok(mtime) = meta.modified() {
                        current.insert(path.clone(), mtime);

                        if !first_pass {
                            let is_new = !mtimes.contains_key(&path);
                            let changed = mtimes.get(&path).is_none_or(|old| *old != mtime);
                            if is_new || changed {
                                let change_type = ChangeType::from_path(&path);
                                broadcaster.broadcast(ReloadEvent { change_type });
                            }
                        }
                    }
                }
            }

            mtimes = current;
            first_pass = false;
        }
    });
}

async fn walk_dir(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];

    while let Some(dir) = dirs.pop() {
        let mut rd = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = rd.next_entry().await? {
            let path = entry.path();
            if entry.file_type().await?.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }

    Ok(files)
}

pub(crate) fn make_html_response(bytes: Vec<u8>, mime: &str) -> Response<ResponseBody> {
    let bytes = if mime.starts_with("text/html") {
        inject_livereload_script(&bytes).unwrap_or(bytes)
    } else {
        bytes
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", mime)
        .body(BoxBody::new(
            Full::new(Bytes::from(bytes)).map_err(|e: std::convert::Infallible| match e {}),
        ))
        .unwrap()
}
