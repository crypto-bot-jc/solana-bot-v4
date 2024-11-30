use std::{
    io,
    io::{Error, ErrorKind},
    net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs},
    panic,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread,
    thread::{sleep, JoinHandle},
    time::Duration,
};

use arc_swap::ArcSwap;
use clap::{arg, Parser, ValueEnum};
use crossbeam_channel::{Receiver, RecvError, Sender};
use log::*;
use signal_hook::consts::{SIGINT, SIGTERM};
use solana_client::client_error::{reqwest, ClientError};
use solana_metrics::set_host_id;
use solana_perf::deduper::Deduper;
use solana_sdk::signature::read_keypair_file;
use thiserror::Error;
use tokio::runtime::Runtime;
use tonic::Status;

use crate::{forwarder::ShredMetrics, token_authenticator::BlockEngineConnectionError, logger::LogMode};

pub mod analyser;
pub mod forwarder;
mod heartbeat;
pub mod logger;
mod token_authenticator;

#[derive(Copy, Clone, Debug, ValueEnum)]
enum LogModeArg {
    /// Disable all logging
    Disabled,
    /// Log to console only
    ConsoleOnly,
    /// Log to file only
    FileOnly,
    /// Log to both console and file
    Both,
}

impl From<LogModeArg> for LogMode {
    fn from(arg: LogModeArg) -> Self {
        match arg {
            LogModeArg::Disabled => LogMode::Disabled,
            LogModeArg::ConsoleOnly => LogMode::ConsoleOnly,
            LogModeArg::FileOnly => LogMode::FileOnly,
            LogModeArg::Both => LogMode::Both,
        }
    }
}

#[derive(Clone, Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    shredstream_args: ProxySubcommands,

    /// Configure logging mode
    #[arg(long, value_enum, default_value = "both")]
    log_mode: LogModeArg,
}

#[derive(Clone, Debug, clap::Subcommand)]
enum ProxySubcommands {
    /// Requests shreds from Jito and sends to all destinations.
    Shredstream(ShredstreamArgs),

    /// Does not request shreds from Jito. Sends anything received on `src-bind-addr`:`src-bind-port` to all destinations.
    ForwardOnly(CommonArgs),
}

#[derive(clap::Args, Clone, Debug)]
struct ShredstreamArgs {
    /// Address for Jito Block Engine.
    /// See https://jito-labs.gitbook.io/mev/searcher-resources/block-engine#connection-details
    #[arg(long, env, default_value = "https://ny.mainnet.block-engine.jito.wtf")]
    block_engine_url: String,

    /// Manual override for auth service address. For internal use.
    #[arg(long, env)]
    auth_url: Option<String>,

    /// Path to keypair file used to authenticate with the backend.
    #[arg(long, env, default_value = "shred.key.json")]
    auth_keypair: PathBuf,

    /// Desired regions to receive heartbeats from.
    /// Receives `n` different streams. Requires at least 1 region, comma separated.
    #[arg(long, env, default_value = "ny", value_delimiter = ',')]
    desired_regions: Vec<String>,

    #[clap(flatten)]
    common_args: CommonArgs,
}

#[derive(clap::Args, Clone, Debug)]
struct CommonArgs {
    /// Address where Shredstream proxy listens.
    #[arg(long, env, default_value_t = IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)))]
    src_bind_addr: IpAddr,

    /// Port where Shredstream proxy listens. Use `0` for random ephemeral port.
    #[arg(long, env, default_value_t = 20_000)]
    src_bind_port: u16,

    /// Interval between logging stats to stdout and influx
    #[arg(long, env, default_value_t = 15_000)]
    metrics_report_interval_ms: u64,

    /// Logs trace shreds to stdout and influx
    #[arg(long, env, default_value_t = false)]
    debug_trace_shred: bool,

    /// Public IP address to use.
    /// Overrides value fetched from `ifconfig.me`.
    #[arg(long, env)]
    public_ip: Option<IpAddr>,

    /// Number of threads to use. Defaults to use up to 4.
    #[arg(long, env)]
    num_threads: Option<usize>,
}

#[derive(Debug, Error)]
pub enum ShredstreamProxyError {
    #[error("TonicError {0}")]
    TonicError(#[from] tonic::transport::Error),
    #[error("GrpcError {0}")]
    GrpcError(#[from] Status),
    #[error("ReqwestError {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("SerdeJsonError {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("RpcError {0}")]
    RpcError(#[from] ClientError),
    #[error("BlockEngineConnectionError {0}")]
    BlockEngineConnectionError(#[from] BlockEngineConnectionError),
    #[error("RecvError {0}")]
    RecvError(#[from] RecvError),
    #[error("IoError {0}")]
    IoError(#[from] io::Error),
    #[error("Shutdown")]
    Shutdown,
}

fn resolve_hostname_port(hostname_port: &str) -> io::Result<(SocketAddr, String)> {
    let socketaddr = hostname_port.to_socket_addrs()?.next().ok_or_else(|| {
        Error::new(
            ErrorKind::AddrNotAvailable,
            format!("Could not find destination {hostname_port}"),
        )
    })?;

    Ok((socketaddr, hostname_port.to_string()))
}

/// Returns public-facing IPV4 address
pub fn get_public_ip() -> reqwest::Result<IpAddr> {
    info!("Requesting public ip from ifconfig.me...");
    let client = reqwest::blocking::Client::builder()
        .local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
        .build()?;
    let response = client.get("https://ifconfig.me").send()?.text()?;
    let public_ip = IpAddr::from_str(&response).unwrap();
    info!("Retrieved public ip: {public_ip:?}");

    Ok(public_ip)
}

// Creates a channel that gets a message every time `SIGINT` is signalled.
fn shutdown_notifier(exit: Arc<AtomicBool>) -> io::Result<(Sender<()>, Receiver<()>)> {
    let (s, r) = crossbeam_channel::bounded(256);
    let mut signals = signal_hook::iterator::Signals::new([SIGINT, SIGTERM])?;

    let s_thread = s.clone();
    thread::spawn(move || {
        for _ in signals.forever() {
            exit.store(true, Ordering::SeqCst);
            // send shutdown signal multiple times since crossbeam doesn't have broadcast channels
            // each thread will consume a shutdown signal
            for _ in 0..256 {
                if s_thread.send(()).is_err() {
                    break;
                }
            }
        }
    });

    Ok((s, r))
}

fn main() -> Result<(), ShredstreamProxyError> {
    env_logger::builder().init();
    let all_args: Args = Args::parse();

    // Initialize logging with the command line argument
    logger::set_log_mode(all_args.log_mode.into());

    let shredstream_args = all_args.shredstream_args.clone();
    // common args
    let args = match all_args.shredstream_args {
        ProxySubcommands::Shredstream(x) => x.common_args,
        ProxySubcommands::ForwardOnly(x) => x,
    };
    set_host_id(hostname::get()?.into_string().unwrap());

    let exit = Arc::new(AtomicBool::new(false));
    let (shutdown_sender, shutdown_receiver) =
        shutdown_notifier(exit.clone()).expect("Failed to set up signal handler");
    let panic_hook = panic::take_hook();
    {
        let exit = exit.clone();
        panic::set_hook(Box::new(move |panic_info| {
            exit.store(true, Ordering::SeqCst);
            let _ = shutdown_sender.send(());
            error!("exiting process");
            sleep(Duration::from_secs(1));
            // invoke the default handler and exit the process
            panic_hook(panic_info);
        }));
    }

    let runtime = Runtime::new()?;
    let (grpc_restart_signal_s, grpc_restart_signal_r) = crossbeam_channel::bounded(1);
    let mut thread_handles = vec![];
    if let ProxySubcommands::Shredstream(args) = shredstream_args {
        let heartbeat_hdl = start_heartbeat(
            args,
            &exit,
            &shutdown_receiver,
            runtime,
            grpc_restart_signal_r,
        );
        thread_handles.push(heartbeat_hdl);
    }

    // share deduper + metrics between forwarder <-> accessory thread
    // use mutex since metrics are write heavy. cheaper than rwlock
    let deduper = Arc::new(RwLock::new(Deduper::<2, [u8]>::new(
        &mut rand::thread_rng(),
        forwarder::DEDUPER_NUM_BITS,
    )));

    let forwarder_hdls = forwarder::start_forwarder_threads(
        args.src_bind_addr,
        args.src_bind_port,
        args.num_threads,
        shutdown_receiver.clone(),
        exit.clone(),
    );

    let metrics = Arc::new(ShredMetrics::new());

    let metrics_hdl = forwarder::start_forwarder_accessory_thread(
        deduper,
        metrics.clone(),
        args.metrics_report_interval_ms,
        grpc_restart_signal_s,
        shutdown_receiver.clone(),
        exit.clone(),
    );
    thread_handles.push(metrics_hdl);

    info!(
        "Shredstream started, listening on {}:{}/udp.",
        args.src_bind_addr, args.src_bind_port
    );

    for thread in thread_handles {
        thread.join().expect("thread panicked");
    }

    info!(
        "Exiting Shredstream, {} received , {} sent successfully, {} failed, {} duplicate shreds.",
        metrics.agg_received_cumulative.load(Ordering::Relaxed),
        metrics
            .agg_success_forward_cumulative
            .load(Ordering::Relaxed),
        metrics.agg_fail_forward_cumulative.load(Ordering::Relaxed),
        metrics.duplicate_cumulative.load(Ordering::Relaxed),
    );
    Ok(())
}

fn start_heartbeat(
    args: ShredstreamArgs,
    exit: &Arc<AtomicBool>,
    shutdown_receiver: &Receiver<()>,
    runtime: Runtime,
    grpc_restart_signal_r: Receiver<()>,
) -> JoinHandle<()> {
    let auth_keypair = Arc::new(
        read_keypair_file(Path::new(&args.auth_keypair)).unwrap_or_else(|e| {
            panic!(
                "Unable to parse keypair file. Ensure that file {:?} is readable. Error: {e}",
                args.auth_keypair
            )
        }),
    );

    heartbeat::heartbeat_loop_thread(
        args.block_engine_url.clone(),
        args.auth_url.unwrap_or(args.block_engine_url),
        auth_keypair,
        args.desired_regions,
        SocketAddr::new(
            args.common_args
                .public_ip
                .unwrap_or_else(|| get_public_ip().unwrap()),
            args.common_args.src_bind_port,
        ),
        runtime,
        "shredstream_proxy".to_string(),
        grpc_restart_signal_r,
        shutdown_receiver.clone(),
        exit.clone(),
    )
}
