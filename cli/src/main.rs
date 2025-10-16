use std::io::Write;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

type Stream = Framed<TcpStream, msgpack_codec::Codec>;

#[tokio::main]
async fn main() -> Result<()> {
    let tcp_stream = TcpStream::connect("127.0.0.1:7172").await?;
    let mut stream = Framed::new(tcp_stream, msgpack_codec::Codec::new());

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
