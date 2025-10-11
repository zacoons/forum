use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

type Stream = Framed<TcpStream, msgpack_codec::Codec>;

#[derive(Debug)]
struct Post {
    id: u64,
    author: u64,
    date: String, // ISO 8601 time-stamp
    title: String,
    msg: String,
}

#[derive(Debug)]
struct Reply {
    id: u64,
    author: u64,
    date: String, // ISO 8601 time-stamp
    msg: String,
}

const RW_BUF_SIZE: usize = 512;
const MAX_MSG_SIZE: usize = 1024;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:7172").await?;

    loop {
        let (tcp_stream, _) = listener.accept().await?;
        let stream = Framed::with_capacity(
            tcp_stream,
            msgpack_codec::Codec {
                max_msg_size: MAX_MSG_SIZE,
                max_depth: 8,
            },
            RW_BUF_SIZE,
        );
        tokio::spawn(handle_client(stream));
    }
}

async fn handle_client(mut stream: Stream) {
    loop {
        match stream.next().await {
            None => {
                println!("Client disconnected");
                return;
            }
            Some(Ok(v)) => {
                handle_cmd(v, &mut stream).await;
            }
            Some(Err(err)) => {
                eprintln!("Error reading from socket: {:?}", err);
                return;
            }
        }
    }
}

async fn handle_cmd<'a>(v: rmpv::Value, stream: &mut Stream) {
    if let Some(cmd) = v["cmd"].as_str() {
        if let Err(err) = match cmd {
            "posts" => handle_posts(stream).await,

            "auth" => handle_auth(&v["args"], stream).await,

            _ => {
                _ = stream.send(rmpv::Value::from("Invalid command")).await;
                eprintln!("Invalid command: {}", cmd);
                return;
            }
        } {
            _ = stream
                .send(rmpv::Value::from(err.to_string().as_bytes()))
                .await;
            eprintln!("{:?}", err);
            return;
        }
    } else {
        _ = stream
            .send(rmpv::Value::from("Badly formatted command"))
            .await;
        eprintln!("Badly formatted command: {}", v);
        return;
    }
}

// struct ResPosts {
//     posts: Vec<Post>,
// }
async fn handle_posts<'a>(stream: &mut Stream) -> Result<()> {
    stream.send(rmpv::Value::from("post1, post2, ...")).await?;
    stream.flush().await?;
    Ok(())
}

// struct ArgsAuth {
//     name: String,
//     passwd: String,
// }
async fn handle_auth<'a>(args: &rmpv::Value, stream: &mut Stream) -> Result<()> {
    let username = match args["name"].as_str() {
        Some(it) => it,
        None => return Err(anyhow!("Missing username")),
    };
    let passwd = match args["passwd"].as_str() {
        Some(it) => it,
        None => return Err(anyhow!("Missing password")),
    };

    if username == "zacoons" && passwd == "a" {
        stream.send(rmpv::Value::from("OK")).await?;
    } else {
        stream.send(rmpv::Value::from("NOAUTH")).await?;
    }
    stream.flush().await?;

    Ok(())
}
