use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;

use crate::asynchronous::transport::AsyncTransport;
use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::StreamExt;
use futures_util::Stream;
use http::HeaderMap;
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tungstenite::client::IntoClientRequest;
use url::Url;

use super::websocket_general::AsyncWebsocketGeneralTransport;

/// An asynchronous websocket transport type.
/// This type only allows for plain websocket
/// connections ("ws://").
#[derive(Clone)]
pub struct WebsocketTransport {
    inner: AsyncWebsocketGeneralTransport,
    base_url: Arc<RwLock<Url>>,
}

impl WebsocketTransport {
    /// Creates a new instance over a request that might hold additional headers and an URL.
    pub async fn new(base_url: Url, headers: Option<HeaderMap>) -> Result<Self> {
        let mut url = base_url;
        url.query_pairs_mut().append_pair("transport", "websocket");
        url.set_scheme("ws").unwrap();

        let mut req = url.clone().into_client_request()?;
        if let Some(map) = headers {
            // SAFETY: this unwrap never panics as the underlying request is just initialized and in proper state
            req.headers_mut().extend(map);
        }

        let (ws_stream, _) = connect_async(req).await?;
        let (sen, rec) = ws_stream.split();

        let inner = AsyncWebsocketGeneralTransport::new(sen, rec).await;
        Ok(WebsocketTransport {
            inner,
            base_url: Arc::new(RwLock::new(url)),
        })
    }

    /// Sends probe packet to ensure connection is valid, then sends upgrade
    /// request
    pub(crate) async fn upgrade(&self) -> Result<()> {
        self.inner.upgrade().await
    }

    pub(crate) async fn poll_next(&self) -> Result<Option<Bytes>> {
        self.inner.poll_next().await
    }
}

#[async_trait]
impl AsyncTransport for WebsocketTransport {
    async fn emit(&self, data: Bytes, is_binary_att: bool) -> Result<()> {
        self.inner.emit(data, is_binary_att).await
    }

    async fn base_url(&self) -> Result<Url> {
        Ok(self.base_url.read().await.clone())
    }

    async fn set_base_url(&self, base_url: Url) -> Result<()> {
        let mut url = base_url;
        if !url
            .query_pairs()
            .any(|(k, v)| k == "transport" && v == "websocket")
        {
            url.query_pairs_mut().append_pair("transport", "websocket");
        }
        url.set_scheme("ws").unwrap();
        *self.base_url.write().await = url;
        Ok(())
    }
}

impl Stream for WebsocketTransport {
    type Item = Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

impl Debug for WebsocketTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncWebsocketTransport")
            .field(
                "base_url",
                &self
                    .base_url
                    .try_read()
                    .map_or("Currently not available".to_owned(), |url| url.to_string()),
            )
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ENGINE_IO_VERSION;
    use std::str::FromStr;

    async fn new() -> Result<WebsocketTransport> {
        let url = crate::test::engine_io_server()?.to_string()
            + "engine.io/?EIO="
            + &ENGINE_IO_VERSION.to_string();
        WebsocketTransport::new(Url::from_str(&url[..])?, None).await
    }

    #[tokio::test]
    async fn websocket_transport_base_url() -> Result<()> {
        let transport = new().await?;
        let mut url = crate::test::engine_io_server()?;
        url.set_path("/engine.io/");
        url.query_pairs_mut()
            .append_pair("EIO", &ENGINE_IO_VERSION.to_string())
            .append_pair("transport", "websocket");
        url.set_scheme("ws").unwrap();
        assert_eq!(transport.base_url().await?.to_string(), url.to_string());
        transport
            .set_base_url(reqwest::Url::parse("https://127.0.0.1")?)
            .await?;
        assert_eq!(
            transport.base_url().await?.to_string(),
            "ws://127.0.0.1/?transport=websocket"
        );
        assert_ne!(transport.base_url().await?.to_string(), url.to_string());

        transport
            .set_base_url(reqwest::Url::parse(
                "http://127.0.0.1/?transport=websocket",
            )?)
            .await?;
        assert_eq!(
            transport.base_url().await?.to_string(),
            "ws://127.0.0.1/?transport=websocket"
        );
        assert_ne!(transport.base_url().await?.to_string(), url.to_string());
        Ok(())
    }

    #[tokio::test]
    async fn websocket_secure_debug() -> Result<()> {
        let mut transport = new().await?;
        assert_eq!(
            format!("{:?}", transport),
            format!(
                "AsyncWebsocketTransport {{ base_url: {:?} }}",
                transport.base_url().await?.to_string()
            )
        );
        println!("{:?}", transport.next().await.unwrap());
        println!("{:?}", transport.next().await.unwrap());
        Ok(())
    }

    /// Spawns a minimal in-process websocket server that accepts a single
    /// connection, immediately closes it with the A2C-SMCP version-handshake
    /// rejection close code (4900, RFC6455 private range) and keeps the socket
    /// open until the client has read the frame. Returns the bound address.
    async fn spawn_close_4900_server() -> std::net::SocketAddr {
        use futures_util::SinkExt;
        use tokio::net::TcpListener;
        use tokio_tungstenite::accept_async;
        use tungstenite::protocol::frame::{coding::CloseCode, CloseFrame};
        use tungstenite::Message;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                if let Ok(mut ws) = accept_async(stream).await {
                    let _ = ws
                        .send(Message::Close(Some(CloseFrame {
                            code: CloseCode::Library(4900),
                            reason: "version handshake rejected".into(),
                        })))
                        .await;
                    // keep the connection alive until the client has read the
                    // close frame and disconnected
                    while let Some(Ok(_)) = ws.next().await {}
                }
            }
        });
        addr
    }

    /// A close frame's numeric code (incl. the RFC6455 private range 4000-4999)
    /// must be surfaced through `poll_next` rather than silently dropped.
    #[tokio::test]
    async fn websocket_close_code_surfaced_via_poll_next() -> Result<()> {
        let addr = spawn_close_4900_server().await;
        let url = Url::parse(&format!("ws://{}/engine.io/", addr))?;
        let transport = WebsocketTransport::new(url, None).await?;

        match transport.poll_next().await {
            Err(crate::error::Error::WebsocketClosed { code, reason }) => {
                assert_eq!(code, 4900);
                assert_eq!(reason, "version handshake rejected");
            }
            other => panic!("expected Err(WebsocketClosed {{ code: 4900, .. }}), got {other:?}"),
        }
        Ok(())
    }

    /// When the server rejects a WS-only connection at the handshake phase by
    /// closing with code 4900, the error must propagate out of
    /// `build_websocket()` so consumers can read it from the `connect()` path.
    #[tokio::test]
    async fn websocket_close_code_surfaced_via_build_handshake() -> Result<()> {
        use crate::asynchronous::ClientBuilder;

        let addr = spawn_close_4900_server().await;
        let url = Url::parse(&format!("ws://{}/", addr))?;

        match ClientBuilder::new(url).build_websocket().await {
            Err(crate::error::Error::WebsocketClosed { code, reason }) => {
                assert_eq!(code, 4900);
                assert_eq!(reason, "version handshake rejected");
            }
            Err(other) => {
                panic!("expected WebsocketClosed {{ code: 4900, .. }}, got Err({other:?})")
            }
            Ok(_) => panic!("expected build_websocket to fail with WebsocketClosed {{ 4900 }}"),
        }
        Ok(())
    }

    /// The production async path drives the transport through its `Stream` impl
    /// (`StreamExt::next`), not the inherent `poll_next`. Cover that branch
    /// directly so the shared close-capture logic in
    /// `AsyncWebsocketGeneralTransport` (reused by both ws and wss) is guarded.
    #[tokio::test]
    async fn websocket_close_code_surfaced_via_stream_impl() -> Result<()> {
        let addr = spawn_close_4900_server().await;
        let url = Url::parse(&format!("ws://{}/engine.io/", addr))?;
        let mut transport = WebsocketTransport::new(url, None).await?;

        match transport.next().await {
            Some(Err(crate::error::Error::WebsocketClosed { code, reason })) => {
                assert_eq!(code, 4900);
                assert_eq!(reason, "version handshake rejected");
            }
            other => {
                panic!("expected Some(Err(WebsocketClosed {{ code: 4900, .. }})), got {other:?}")
            }
        }
        Ok(())
    }
}
