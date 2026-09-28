#![cfg(feature = "async")]

use futures_util::FutureExt;
use http_body_util::Full;
use hyper::body::Bytes;
use socketioxide::SocketIo;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tf_rust_socketio::{
    asynchronous::{ClientBuilder, ReconnectSettings},
    Event, TransportType,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::{AbortHandle, JoinHandle},
    time::timeout,
};
use tower::Layer;

// Real Socket.IO over a TCP proxy: dropping both proxy streams forces a transport
// loss, without substituting the protocol or reconnect implementation under test.
struct Server {
    url: String,
    streams: Arc<Mutex<Vec<AbortHandle>>>,
    backend: JoinHandle<()>,
    proxy: JoinHandle<()>,
    io: SocketIo,
    closed: mpsc::UnboundedReceiver<()>,
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (layer, io) = SocketIo::new_layer();
        let (closed_tx, closed) = mpsc::unbounded_channel();
        io.ns("/", move |socket: socketioxide::extract::SocketRef| {
            let tx = closed_tx.clone();
            socket.on_disconnect(move || {
                let _ = tx.send(());
            });
            socket.emit("disconnect_me", &()).unwrap();
        });
        let fallback = tower::service_fn(|_| async {
            Ok::<_, std::convert::Infallible>(hyper::Response::new(Full::new(Bytes::new())))
        });
        let service = layer.layer(fallback);
        let backend = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let service = hyper_util::service::TowerToHyperService::new(service.clone());
                tokio::spawn(async move {
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
                        .with_upgrades()
                        .await;
                });
            }
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let streams = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::clone(&streams);
        let proxy = tokio::spawn(async move {
            loop {
                let (mut downstream, _) = listener.accept().await.unwrap();
                let task = tokio::spawn(async move {
                    let mut upstream = TcpStream::connect(address).await.unwrap();
                    let _ = tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await;
                });
                connections.lock().unwrap().push(task.abort_handle());
            }
        });
        Self {
            url,
            streams,
            backend,
            proxy,
            io,
            closed,
        }
    }

    fn cut_transport(&self) {
        for task in self.streams.lock().unwrap().drain(..) {
            task.abort();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.cut_transport();
        self.backend.abort();
        self.proxy.abort();
        let io = self.io.clone();
        tokio::spawn(async move {
            io.close().await;
        });
    }
}

struct OnDrop(Option<oneshot::Sender<()>>);
impl Drop for OnDrop {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

#[tokio::test]
async fn failed_close_frame_still_invalidates_engineio_connection() {
    let mut server = Server::start().await;
    let url = url::Url::parse(&format!("{}/socket.io/", server.url)).unwrap();
    let client = tf_rust_engineio::asynchronous::ClientBuilder::new(url)
        .build_polling()
        .await
        .unwrap();
    client.connect().await.unwrap();
    let alias = client.clone();
    assert!(alias.is_connected());

    // No reader is polling this client: only disconnect can invalidate its
    // local state. Close the listener too, so the CLOSE POST must fail.
    server.proxy.abort();
    let _ = (&mut server.proxy).await;
    server.cut_transport();
    assert!(timeout(Duration::from_secs(2), client.disconnect())
        .await
        .unwrap()
        .is_err());
    assert!(!alias.is_connected());
}

#[tokio::test]
async fn disconnect_cancels_pending_reconnect_callback() {
    let server = Server::start().await;
    let (entered_tx, mut entered_rx) = mpsc::unbounded_channel();
    let (dropped_tx, dropped_rx) = oneshot::channel();
    let mut dropped_tx = Some(dropped_tx);
    let client = ClientBuilder::new(&server.url)
        .on_reconnect(move || {
            let guard = OnDrop(dropped_tx.take());
            let tx = entered_tx.clone();
            async move {
                let _guard = guard;
                tx.send(()).unwrap();
                std::future::pending::<ReconnectSettings>().await
            }
            .boxed()
        })
        .connect()
        .await
        .unwrap();
    server.cut_transport();
    timeout(Duration::from_secs(3), entered_rx.recv())
        .await
        .unwrap()
        .unwrap();
    // A dead transport may reject the DISCONNECT frame; cancellation still must run.
    let _ = timeout(Duration::from_secs(2), client.disconnect())
        .await
        .unwrap();
    timeout(Duration::from_secs(1), dropped_rx)
        .await
        .expect("disconnect must drop the pending reconnect future")
        .unwrap();
}

#[tokio::test]
async fn disconnect_stops_reconnect_backoff() {
    let server = Server::start().await;
    let (attempt_tx, mut attempt_rx) = mpsc::unbounded_channel();
    let client = ClientBuilder::new(&server.url)
        .reconnect_delay(100, 100)
        .on_reconnect(move || {
            let tx = attempt_tx.clone();
            async move {
                tx.send(()).unwrap();
                let mut settings = ReconnectSettings::new();
                // URL parsing fails without an I/O yield. On this single-thread
                // runtime the next yield after the signal is the backoff sleep.
                settings.address("invalid URL");
                settings
            }
            .boxed()
        })
        .connect()
        .await
        .unwrap();
    server.cut_transport();
    timeout(Duration::from_secs(3), attempt_rx.recv())
        .await
        .unwrap()
        .unwrap();
    let _ = timeout(Duration::from_secs(2), client.disconnect())
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_millis(400), attempt_rx.recv())
            .await
            .map_or(true, |message| message.is_none()),
        "a retired client must not start another reconnect"
    );
}

#[tokio::test]
async fn disconnect_cancels_pending_http_handshake() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let server = Server::start().await;
    let stalled = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let stalled_url = format!("http://{}", stalled.local_addr().unwrap());
    let (entered_tx, entered_rx) = oneshot::channel();
    let (closed_tx, closed_rx) = oneshot::channel();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = stalled.accept().await.unwrap();
        let mut byte = [0];
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).await.unwrap();
            request.push(byte[0]);
        }
        entered_tx.send(()).unwrap();
        // Deliberately withhold the handshake response. Cancellation must close
        // this real connection, not merely suppress a later namespace callback.
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest).await;
        let _ = stream.shutdown().await;
        let _ = closed_tx.send(());
    });
    let client = ClientBuilder::new(&server.url)
        .transport_type(TransportType::Polling)
        .on_reconnect(move || {
            let url = stalled_url.clone();
            async move {
                let mut settings = ReconnectSettings::new();
                settings.address(url);
                settings
            }
            .boxed()
        })
        .connect()
        .await
        .unwrap();
    server.cut_transport();
    timeout(Duration::from_secs(3), entered_rx)
        .await
        .unwrap()
        .unwrap();
    let _ = timeout(Duration::from_secs(2), client.disconnect())
        .await
        .unwrap();
    let result = timeout(Duration::from_secs(1), closed_rx).await;
    peer.abort();
    result
        .expect("disconnect must cancel the in-flight HTTP handshake")
        .unwrap();
}

#[tokio::test]
async fn transport_loss_still_reconnects_until_explicitly_disconnected() {
    let server = Server::start().await;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = ClientBuilder::new(&server.url)
        .on(Event::Connect, move |_, _| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(());
            }
            .boxed()
        })
        .connect()
        .await
        .unwrap();
    timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    server.cut_transport();
    timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(client.session_epoch() >= 2);
    client.disconnect().await.unwrap();
    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn disconnect_from_application_callback_finishes_teardown() {
    let mut server = Server::start().await;
    let client = ClientBuilder::new(&server.url)
        .on("disconnect_me", |_, client| {
            async move {
                // Stopping the reader cancels this non-terminal dispatch task.
                // Teardown must still reach the peer even when its caller is gone.
                let _ = client.disconnect().await;
            }
            .boxed()
        })
        .connect()
        .await
        .unwrap();
    timeout(Duration::from_secs(2), server.closed.recv())
        .await
        .expect("callback cancellation must not interrupt transport teardown")
        .unwrap();
    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn cancelled_disconnect_caller_does_not_cancel_teardown() {
    let mut server = Server::start().await;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = ClientBuilder::new(&server.url)
        .on(Event::Connect, move |_, _| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(());
            }
            .boxed()
        })
        .connect()
        .await
        .unwrap();
    timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    {
        let disconnect = client.disconnect();
        tokio::pin!(disconnect);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(disconnect.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
    }
    timeout(Duration::from_secs(2), server.closed.recv())
        .await
        .expect("dropping disconnect must not abandon cleanup")
        .unwrap();
    let alias = client.clone();
    let (first, second) = tokio::join!(client.disconnect(), alias.disconnect());
    first.unwrap();
    second.unwrap();
}
