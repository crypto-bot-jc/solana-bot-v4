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
use crossbeam_channel::{Receiver, RecvError, Sender};
use log::*;
use signal_hook::consts::{SIGINT, SIGTERM};
use solana_client::client_error::{reqwest, ClientError};
use solana_metrics::set_host_id;
use solana_perf::deduper::Deduper;
use solana_sdk::signature::read_keypair_file;
use thiserror::Error;
use tonic::Status;
use tokio::runtime::Runtime;

use crate::shredstream::{
    forwarder::{ShredMetrics, self},
    token_authenticator::BlockEngineConnectionError,
    logger::{self, LogMode},
    heartbeat,
};

#[derive(Clone, Debug)]
pub struct ShredstreamConfig {
    pub log_mode: LogMode,
    pub block_engine_url: String,
    pub auth_url: Option<String>,
    pub auth_keypair: PathBuf,
    pub desired_regions: Vec<String>,
    pub src_bind_addr: IpAddr,
    pub src_bind_port: u16,
    pub metrics_report_interval_ms: u64,
    pub debug_trace_shred: bool,
    pub public_ip: Option<IpAddr>,
    pub num_threads: Option<usize>,
}

impl Default for ShredstreamConfig {
    fn default() -> Self {
        Self {
            log_mode: LogMode::Both,
            block_engine_url: "https://ny.mainnet.block-engine.jito.wtf".to_string(),
            auth_url: None,
            auth_keypair: PathBuf::from("shred.key.json"),
            desired_regions: vec!["ny".to_string()],
            src_bind_addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            src_bind_port: 20_000,
            metrics_report_interval_ms: 15_000,
            debug_trace_shred: false,
            public_ip: None,
            num_threads: None,
        }
    }
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

pub fn resolve_hostname_port(hostname_port: &str) -> io::Result<(SocketAddr, String)> {
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

pub fn run_shredstream(config: ShredstreamConfig) -> Result<(), ShredstreamProxyError> {
    // Create the runtime outside of any async context
    let runtime = Runtime::new().expect("Failed to create Tokio runtime");
    
    env_logger::builder().init();

    // Initialize logging
    logger::set_log_mode(config.log_mode);

    println!(
        "Starting Shredstream with desired regions: {} and block engine url: {}",
        config.desired_regions.join(", "),
        config.block_engine_url
    );

    set_host_id(hostname::get()?.into_string().unwrap());

    let exit = Arc::new(AtomicBool::new(false));
    let (shutdown_sender, shutdown_receiver) =
        shutdown_notifier(exit.clone()).expect("Failed to set up signal handler");
    let panic_hook = panic::take_hook();
    {
        let exit = exit.clone();
        let shutdown_sender = shutdown_sender.clone();
        panic::set_hook(Box::new(move |panic_info| {
            exit.store(true, Ordering::SeqCst);
            let _ = shutdown_sender.send(());
            error!("exiting process");
            sleep(Duration::from_secs(1));
            // invoke the default handler and exit the process
            panic_hook(panic_info);
        }));
    }

    let (grpc_restart_signal_s, grpc_restart_signal_r) = crossbeam_channel::bounded(1);
    let mut thread_handles = vec![];

    // Initialize shared state
    let deduper = Arc::new(RwLock::new(Deduper::<2, [u8]>::new(
        &mut rand::thread_rng(),
        forwarder::DEDUPER_NUM_BITS,
    )));
    let metrics = Arc::new(ShredMetrics::new());
    let unioned_dest_sockets = Arc::new(ArcSwap::new(Arc::new(vec![])));

    // Start heartbeat
    let heartbeat_hdl = start_heartbeat(
        &config,
        &exit,
        &shutdown_receiver,
        grpc_restart_signal_r,
    );
    thread_handles.push(heartbeat_hdl);

    // Start forwarder threads
    let forwarder_hdls = forwarder::start_forwarder_threads(
        unioned_dest_sockets.clone(),
        config.src_bind_addr,
        config.src_bind_port,
        config.num_threads,
        deduper.clone(),
        metrics.clone(),
        true,
        config.debug_trace_shred,
        shutdown_receiver.clone(),
        exit.clone(),
    );
    thread_handles.extend(forwarder_hdls);

    // Start metrics thread
    let metrics_hdl = forwarder::start_forwarder_accessory_thread(
        deduper,
        metrics.clone(),
        config.metrics_report_interval_ms,
        grpc_restart_signal_s,
        shutdown_receiver.clone(),
        exit.clone(),
    );
    thread_handles.push(metrics_hdl);

    info!(
        "Shredstream started, listening on {}:{}/udp.",
        config.src_bind_addr, config.src_bind_port
    );

    // Wait for all threads to complete
    for thread in thread_handles {
        thread.join().expect("thread panicked");
    }

    // Ensure runtime is dropped outside of any async context
    drop(runtime);

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
    config: &ShredstreamConfig,
    exit: &Arc<AtomicBool>,
    shutdown_receiver: &Receiver<()>,
    grpc_restart_signal_r: Receiver<()>,
) -> JoinHandle<()> {
    let auth_keypair = Arc::new(
        read_keypair_file(Path::new(&config.auth_keypair)).unwrap_or_else(|e| {
            panic!(
                "Unable to parse keypair file. Ensure that file {:?} is readable. Error: {e}",
                config.auth_keypair
            )
        }),
    );

    let socket_addr = SocketAddr::new(
        config.public_ip.unwrap_or_else(|| get_public_ip().unwrap()),
        config.src_bind_port,
    );

    let block_engine_url = config.block_engine_url.clone();
    let auth_url = config.auth_url.clone().unwrap_or(config.block_engine_url.clone());
    let desired_regions = config.desired_regions.clone();

    heartbeat::heartbeat_loop_thread(
        block_engine_url,
        auth_url,
        auth_keypair,
        desired_regions,
        socket_addr,
        "shredstream_proxy".to_string(),
        grpc_restart_signal_r,
        shutdown_receiver.clone(),
        exit.clone(),
    )
}

pub fn main() -> Result<(), ShredstreamProxyError> {
    // Use default configuration
    let config = ShredstreamConfig::default();
    run_shredstream(config)
}
