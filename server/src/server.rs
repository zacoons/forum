use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream, ToSocketAddrs},
};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
    },
    server::TlsStream,
};
use tokio_util::codec::Framed;

pub type Stream = Framed<InnerStream, msgpack_codec::Codec>;

#[derive(Debug)]
pub enum InnerStream {
    Insecure(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl AsyncRead for InnerStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            InnerStream::Insecure(tcp_stream) => Pin::new(tcp_stream).poll_read(cx, buf),
            InnerStream::Tls(tls_stream) => Pin::new(tls_stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for InnerStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        match &mut *self {
            InnerStream::Insecure(tcp_stream) => Pin::new(tcp_stream).poll_write(cx, buf),
            InnerStream::Tls(tls_stream) => Pin::new(tls_stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        match &mut *self {
            InnerStream::Insecure(tcp_stream) => Pin::new(tcp_stream).poll_flush(cx),
            InnerStream::Tls(tls_stream) => Pin::new(tls_stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        match &mut *self {
            InnerStream::Insecure(tcp_stream) => Pin::new(tcp_stream).poll_shutdown(cx),
            InnerStream::Tls(tls_stream) => Pin::new(tls_stream).poll_shutdown(cx),
        }
    }
}

pub struct Server {
    max_msg_size: usize,
    rw_buf_size: usize,
    tls: Option<TlsAcceptor>,
    listener: TcpListener,
}

impl Server {
    pub async fn bind<A>(max_msg_size: usize, rw_buf_size: usize, addr: A) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        let listener = TcpListener::bind(&addr).await?;

        Ok(Self {
            max_msg_size,
            rw_buf_size,
            tls: None,
            listener,
        })
    }

    pub async fn bind_with_tls<A>(
        max_msg_size: usize,
        rw_buf_size: usize,
        addr: A,
        cert_path: impl AsRef<std::path::Path>,
        key_path: impl AsRef<std::path::Path>,
    ) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        let certs = CertificateDer::pem_file_iter(cert_path)?.collect::<Result<Vec<_>, _>>()?;
        let key = PrivateKeyDer::from_pem_file(key_path)?;
        let tls = Some(TlsAcceptor::from(Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)?,
        )));

        let listener = TcpListener::bind(&addr).await?;

        Ok(Self {
            max_msg_size,
            rw_buf_size,
            tls,
            listener,
        })
    }

    pub async fn accept(&self) -> Result<Framed<InnerStream, msgpack_codec::Codec>> {
        let (stream, _) = self.listener.accept().await?;
        let stream = if let Some(tls) = &self.tls {
            InnerStream::Tls(tls.clone().accept(stream).await?)
        } else {
            InnerStream::Insecure(stream)
        };
        let stream = Framed::with_capacity(
            stream,
            msgpack_codec::Codec {
                max_msg_size: self.max_msg_size,
                max_depth: 8,
            },
            self.rw_buf_size,
        );
        Ok(stream)
    }
}
