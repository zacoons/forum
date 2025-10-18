use std::{
    io::Write,
    net::{IpAddr, ToSocketAddrs},
    str::FromStr,
    sync::Arc,
};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tokio_rustls::{
    TlsConnector,
    client::TlsStream,
    rustls::{
        self, SignatureScheme,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, DnsName, ServerName},
    },
};
use tokio_util::codec::Framed;

type Stream = Framed<TlsStream<TcpStream>, msgpack_codec::Codec>;

#[derive(Deserialize)]
struct Conf {
    addr: String,
    port: Option<u16>,
}

#[derive(Debug)]
struct NoCertificateVerification {}
impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
            SignatureScheme::ML_DSA_44,
            SignatureScheme::ML_DSA_65,
            SignatureScheme::ML_DSA_87,
        ]
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let conf: Conf = toml::from_slice(&std::fs::read("conf.toml")?)?;

    let addr = Box::new(conf.addr).leak();
    let port = conf.port.unwrap_or(7172);
    let server_name = if let Ok(ip_addr) = IpAddr::from_str(addr) {
        ServerName::IpAddress(ip_addr.into())
    } else {
        ServerName::DnsName(DnsName::try_from_str(addr)?)
    };

    let mut root_cert_store = rustls::RootCertStore::empty();
    // TODO: Read this from the system.
    // See Bundle.rescan in https://github.com/ziglang/zig/blob/master/lib/std/crypto/Certificate/Bundle.zig
    root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    // vvv FOR TESTING ONLY vvv
    client_config
        .dangerous()
        .set_certificate_verifier(Arc::new(NoCertificateVerification {}));
    // ^^^ ALERT ^^^
    let connector = TlsConnector::from(Arc::new(client_config));

    let socket_addr = (addr.to_owned(), port).to_socket_addrs()?.next().unwrap();
    let tcp_stream = TcpStream::connect(socket_addr).await?;

    let tls_stream = connector.connect(server_name, tcp_stream).await?;
    let mut stream = Framed::new(tls_stream, msgpack_codec::Codec::new());

    let mut args = std::env::args();
    _ = args.next();
    if let Some(cmd) = args.next() {
        match cmd.as_str() {
            "posts" => posts(&mut stream).await?,
            "auth" => auth(&mut stream).await?,
            _ => print_help(),
        }
    } else {
        print_help();
    }

    stream.into_inner().shutdown().await?;

    Ok(())
}

fn print_help() {
    println!("help")
}

async fn posts(stream: &mut Stream) -> Result<()> {
    let command = rmpv::Value::Map(vec![(rmpv::Value::from("cmd"), rmpv::Value::from("posts"))]);
    stream.send(command).await?;
    stream.flush().await?;

    let response = stream.next().await.unwrap()?;
    println!("{}", response);

    Ok(())
}

async fn auth(stream: &mut Stream) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(b"name: ")?;
    stdout.flush()?;
    let mut name = String::new();
    std::io::stdin().read_line(&mut name)?;

    let mut stdout = std::io::stdout();
    stdout.write_all(b"passwd: ")?;
    stdout.flush()?;
    let mut passwd = String::new();
    std::io::stdin().read_line(&mut passwd)?;

    let args = rmpv::Value::Map(vec![
        (
            rmpv::Value::from("name"),
            rmpv::Value::from(&name[..name.len() - 1]),
        ),
        (
            rmpv::Value::from("passwd"),
            rmpv::Value::from(&passwd[..passwd.len() - 1]),
        ),
    ]);
    let command = rmpv::Value::Map(vec![
        (rmpv::Value::from("cmd"), rmpv::Value::from("auth")),
        (rmpv::Value::from("args"), args),
    ]);
    stream.send(command).await?;

    let response = stream.next().await.unwrap()?;
    if let Some(tok) = response["tok"].as_str() {
        std::fs::write("tok", tok)?;
        println!("Success");
    } else if let Some(err) = response["err"].as_str() {
        println!("Error: {}", err);
    } else {
        println!("Unknown error");
    }

    Ok(())
}
