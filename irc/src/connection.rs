use std::path::PathBuf;

use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_util::codec;
use tokio_util::codec::Framed;

mod tls;

pub enum Connection<Codec> {
    Tls(Framed<TlsStream<TcpStream>, Codec>),
    Unsecured(Framed<TcpStream, Codec>),
    // Stdio(Framed<io::Join<io::Stdin, io::Stdout>, Codec>),
}

#[derive(Debug, Clone)]
pub enum Security<'a> {
    Unsecured,
    Secured {
        root_cert_path: Option<&'a PathBuf>,
        client_cert_path: Option<&'a PathBuf>,
        client_key_path: Option<&'a PathBuf>,
    },
}

#[derive(Debug, Clone)]
pub struct Config<'a> {
    pub server: &'a str,
    pub port: u16,
    pub security: Security<'a>,
}

impl<Codec> Connection<Codec> {
    pub async fn new(config: Config<'_>, codec: Codec) -> Result<Self, Error> {
        let tcp = TcpStream::connect((config.server, config.port)).await?;
        tcp.set_nodelay(true).unwrap();

        if let Security::Secured {
            root_cert_path,
            client_cert_path,
            client_key_path,
        } = config.security
        {
            let tls = tls::connect(
                tcp,
                config.server,
                root_cert_path,
                client_cert_path,
                client_key_path,
            )
            .await?;

            Ok(Self::Tls(Framed::new(tls, codec)))
        } else {
            Ok(Self::Unsecured(Framed::new(tcp, codec)))
        }
    }

    pub async fn shutdown(self) -> Result<(), Error> {
        match self {
            Connection::Tls(framed) => {
                framed.into_inner().shutdown().await?;
            }
            Connection::Unsecured(framed) => {
                framed.into_inner().shutdown().await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tls error: {0}")]
    Tls(#[from] tls::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

macro_rules! delegate {
    ($e:expr, $($t:tt)*) => {
        match $e {
            $crate::connection::Connection::Tls(framed) => framed.$($t)*,
            $crate::connection::Connection::Unsecured(framed) => framed.$($t)*,
        }
    };
}

impl<Codec> Stream for Connection<Codec>
where
    Codec: codec::Decoder,
{
    type Item = Result<Codec::Item, Codec::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        delegate!(self.get_mut(), poll_next_unpin(cx))
    }
}

impl<Item, Codec> Sink<Item> for Connection<Codec>
where
    Codec: codec::Encoder<Item>,
{
    type Error = Codec::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        delegate!(self.get_mut(), poll_ready_unpin(cx))
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        delegate!(self.get_mut(), start_send_unpin(item))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        delegate!(self.get_mut(), poll_flush_unpin(cx))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        delegate!(self.get_mut(), poll_close_unpin(cx))
    }
}
