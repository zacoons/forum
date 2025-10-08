use anyhow::{Result, anyhow};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};

const RW_BUF_SIZE: usize = 512;
const MAX_MSG_SIZE: usize = 1024;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:7172").await?;

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(handle_client(stream));
    }
}

struct StackVec<T, const N: usize> {
    buf: [T; N],
    pub len: usize,
}

impl<T, const N: usize> StackVec<T, N>
where
    T: Copy + Sized,
{
    pub fn new(buf: [T; N]) -> Self {
        Self { buf: buf, len: 0 }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.buf[..self.len]
    }

    pub fn append(&mut self, other: &[T]) {
        self.buf[self.len..(self.len + other.len())].copy_from_slice(other);
        self.len += other.len();
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }
}

async fn handle_client(mut stream: TcpStream) {
    let mut readbuf = [0; RW_BUF_SIZE];
    let mut cmdbuf = StackVec::new([0; MAX_MSG_SIZE]);

    let (mut reader, mut writer) = stream.split();

    loop {
        let n = match reader.read(&mut readbuf).await {
            Ok(0) => {
                println!("Client disconnected");
                return;
            }
            Ok(n) => n,
            Err(err) => {
                eprintln!("Error reading from socket: {:?}", err);
                return;
            }
        };
        let mut prevcmd = 0;
        let mut i = 0;
        while i <= n - 4 {
            // Upon encountering an end delimiter:
            if &readbuf[i..i + 4] == b"\0end" {
                // Finish the cmdbuf and use it to call handle_cmd
                cmdbuf.append(&readbuf[prevcmd..i]);
                handle_cmd(&mut reader, &mut writer, &cmdbuf.as_slice()).await;

                // Clear the cmdbuf to store the next command
                cmdbuf.clear();

                // Store the index into buf where this command ends
                // so that we know where to start reading the next one
                prevcmd = i + 4;
                i += 4;
            }
            i += 1;
        }
        i -= 1;
        if i < n {
            cmdbuf.append(&readbuf[i..n]);
        }
    }
}

async fn handle_cmd<'a>(reader: &mut ReadHalf<'a>, writer: &mut WriteHalf<'a>, cmdbuf: &[u8]) {
    // At most 8 parts
    let mut parts = cmdbuf.splitn(8, |&c| c == b'\0');

    if let Some(cmd) = parts.next() {
        if let Err(err) = match cmd {
            b"posts" => handle_posts(reader, writer).await,

            b"auth" => handle_auth(reader, writer, parts).await,

            _ => {
                _ = writer.write_all(b"Invalid command").await;
                if let Ok(cmdstr) = str::from_utf8(cmd) {
                    eprintln!("Invalid command: {}", cmdstr);
                } else {
                    eprintln!("Invalid command: {:?}", cmd);
                }
                return;
            }
        } {
            _ = writer.write_all(err.to_string().as_bytes()).await;
            eprintln!("{:?}", err);
            return;
        }
    } else {
        _ = writer.write_all(b"Badly formatted command").await;
        if let Ok(bufstr) = str::from_utf8(cmdbuf) {
            eprintln!("Badly formatted command: {}", bufstr);
        } else {
            eprintln!("Badly formatted command: {:?}", cmdbuf);
        }
        return;
    }
}

async fn handle_posts<'a>(reader: &mut ReadHalf<'a>, writer: &mut WriteHalf<'a>) -> Result<()> {
    writer.write_all(b"post1, post2, ...").await?;
    writer.flush().await?;
    Ok(())
}

async fn handle_auth<'a>(
    reader: &mut ReadHalf<'a>,
    writer: &mut WriteHalf<'a>,
    mut args: impl Iterator<Item = &[u8]>,
) -> Result<()> {
    let username = match args.next() {
        Some(it) => it,
        None => return Err(anyhow!("Missing username")),
    };
    let passwd = match args.next() {
        Some(it) => it,
        None => return Err(anyhow!("Missing password")),
    };

    if username == b"zacoons" && passwd == b"a" {
        writer.write_all(b"OK").await?;
    } else {
        writer.write_all(b"NOAUTH").await?;
    }
    writer.flush().await?;

    Ok(())
}
