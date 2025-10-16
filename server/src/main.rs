use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use uuid::Uuid;

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

struct Context {
    conf: &'static toml::Table,
    auth_tokens: Arc<Mutex<HashMap<String, String>>>,
    stream: Framed<TcpStream, msgpack_codec::Codec>,
}

const RW_BUF_SIZE: usize = 512;
const MAX_MSG_SIZE: usize = 1024;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args();
    _ = args.next();
    if let Some(cmd) = args.next() {
        match cmd.as_str() {
            "passwd" => {
                let mut passwd = String::new();
                std::io::stdin().read_line(&mut passwd)?;
                println!(
                    "{}",
                    bcrypt::hash(&passwd[..passwd.len() - 1], bcrypt::DEFAULT_COST)?
                );
                return Ok(());
            }
            _ => {}
        }
    }

    let conf_box = Box::new(toml::from_slice::<toml::Table>(&std::fs::read(
        "conf.toml",
    )?)?);
    let conf = Box::leak(conf_box);

    let auth_tokens = Arc::new(Mutex::new(HashMap::<String, String>::new()));

    let listener = TcpListener::bind(conf["addr"].as_str().unwrap()).await?;

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
        let cx = Context {
            conf: conf,
            auth_tokens: auth_tokens.clone(),
            stream: stream,
        };
        tokio::spawn(handle_client(cx));
    }
}

async fn handle_client(mut cx: Context) {
    loop {
        match cx.stream.next().await {
            None => {
                println!("Client disconnected");
                return;
            }
            Some(Ok(v)) => {
                handle_cmd(v, &mut cx).await;
            }
            Some(Err(err)) => {
                eprintln!("Error reading from socket: {:?}", err);
                return;
            }
        }
    }
}

async fn handle_cmd<'a>(v: rmpv::Value, cx: &mut Context) {
    if let Some(cmd) = v["cmd"].as_str() {
        if let Err(err) = match cmd {
            "posts" => handle_posts(cx).await,

            "auth" => handle_auth(&v["args"], cx).await,

            _ => {
                _ = cx.stream.send(rmpv::Value::from("Invalid command")).await;
                eprintln!("Invalid command: {}", cmd);
                return;
            }
        } {
            _ = cx
                .stream
                .send(rmpv::Value::from(err.to_string().as_bytes()))
                .await;
            eprintln!("{:?}", err);
            return;
        }
    } else {
        _ = cx
            .stream
            .send(rmpv::Value::from("Badly formatted command"))
            .await;
        eprintln!("Badly formatted command: {}", v);
        return;
    }
}

async fn handle_posts<'a>(cx: &mut Context) -> Result<()> {
    cx.stream
        .send(rmpv::Value::from("post1, post2, ..."))
        .await?;
    cx.stream.flush().await?;
    Ok(())
}

async fn handle_auth<'a>(args: &rmpv::Value, cx: &mut Context) -> Result<()> {
    let name = match args["name"].as_str() {
        Some(it) => it,
        None => return Err(anyhow!("Missing name")),
    };
    let passwd = match args["passwd"].as_str() {
        Some(it) => it,
        None => return Err(anyhow!("Missing password")),
    };

    if verify_user(name, passwd, cx)? {
        let tok = Uuid::now_v7().to_string();
        cx.auth_tokens
            .lock()
            .unwrap()
            .insert(name.to_owned(), tok.clone());
        cx.stream
            .send(rmpv::Value::from(vec![(
                rmpv::Value::from("tok"),
                rmpv::Value::from(tok),
            )]))
            .await?;
    } else {
        cx.stream
            .send(rmpv::Value::from(vec![(
                rmpv::Value::from("err"),
                rmpv::Value::from("not_authenticated"),
            )]))
            .await?;
    }
    cx.stream.flush().await?;

    Ok(())
}
fn verify_user(name: &str, passwd: &str, cx: &mut Context) -> Result<bool> {
    if let Some(users) = cx.conf.get("users")
        && let Some(user) = users.get(name)
        && let Some(user) = user.as_table()
    {
        if let Some(hash) = user.get("passwd")
            && let Some(hash) = hash.as_str()
        {
            return Ok(bcrypt::verify(passwd, hash)?);
        }
    }
    Ok(false)
}
