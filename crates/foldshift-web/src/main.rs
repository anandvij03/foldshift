use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "foldshift-web",
    version,
    about = "Lightweight local web UI for FoldShift"
)]
struct Args {
    /// IP address to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Do not automatically open the browser on start
    #[arg(long)]
    no_open: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    foldshift_web::run(addr, !args.no_open).await
}
