use std::{
    collections::HashMap, net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket}, panic, sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, RwLock, Mutex,
    }, thread::{sleep, spawn, Builder, JoinHandle}, time::{Duration, SystemTime}
};

use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, RecvError, Sender, TrySendError};
use dashmap::DashMap;
use solana_ledger::shred::Shred;
use solana_ledger::shred::ShredData;
use solana_ledger::shred::ReedSolomonCache;
use solana_entry::entry::Entry;

use bincode::{deserialize, Error};

use itertools::Itertools;

use crate::analyser::{self, recv_from_channel_and_analyse_shred}; 
use jito_protos::trace_shred::TraceShred;

use log::{debug, error, info, warn};
use solana_sdk::transaction::VersionedTransaction;
use prost::Message;
use solana_metrics::{datapoint_info, datapoint_warn};
use solana_perf::{
    deduper::Deduper,
    packet::{PacketBatch, PacketBatchRecycler, Packet},
    recycler::Recycler,
};
use solana_streamer::{
    sendmmsg::{batch_send, SendPktsError},
    streamer,
    streamer::StreamerReceiveStats,
};

use crate::{resolve_hostname_port, ShredstreamProxyError};

pub const DEDUPER_FALSE_POSITIVE_RATE: f64 = 0.001;
pub const DEDUPER_NUM_BITS: u64 = 637_534_199; // 76MB
pub const DEDUPER_RESET_CYCLE: Duration = Duration::from_secs(5 * 60);

struct Shredsreceived {
    shreds: Vec<Shred>
}

impl Shredsreceived {
    fn new() -> Shredsreceived {
        Shredsreceived {
            shreds: Vec::new(),
        }
    }
}

/// Bind to port and start forwarding shreds using a single connection
#[allow(clippy::too_many_arguments)]
pub fn start_forwarder_threads(
    src_addr: IpAddr,
    src_port: u16,
    num_threads: Option<usize>,
    shutdown_receiver: Receiver<()>,
    exit: Arc<AtomicBool>,
) -> Vec<JoinHandle<()>> {
    let recycler: PacketBatchRecycler = Recycler::warmed(100, 1024);
    
    // Create a single socket connection
    let incoming_shred_socket = UdpSocket::bind(SocketAddr::new(src_addr, src_port))
        .unwrap_or_else(|_| {
            panic!("Failed to bind listener socket. Check that port {src_port} is not in use.")
        });

    let (packet_sender, packet_receiver) = crossbeam_channel::unbounded();
    let stats = Arc::new(StreamerReceiveStats::new("shredstream_proxy-listen_thread"));
    
    // Create single listen thread
    let listen_thread = streamer::receiver(
        Arc::new(incoming_shred_socket),
        exit.clone(),
        packet_sender,
        recycler.clone(),
        stats.clone(),
        Duration::default(),
        false,
        Some(Arc::new(AtomicBool::new(false))),
        false,
    );

    let report_metrics_thread = {
        let exit = exit.clone();
        spawn(move || {
            while !exit.load(Ordering::Relaxed) {
                sleep(Duration::from_secs(1));
                stats.report();
            }
        })
    };

    let shutdown_receiver = shutdown_receiver.clone();
    let exit = exit.clone();

    let mut shred_map: HashMap<(u64, u32), Vec<Shred>> = HashMap::new();
    let mut total_shred_received_count = 0;
    let shreds_to_ignore = Arc::new(Mutex::new(Vec::new()));
    
    // Create single processing thread
    let process_thread = Builder::new()
        .name("ssPxyTx_main".to_string())
        .spawn(move || {
            while !exit.load(Ordering::Relaxed) {
                crossbeam_channel::select! {
                    // forward packets
                    recv(packet_receiver) -> maybe_packet_batch => {
                        let res = recv_from_channel_and_analyse_shred(
                            maybe_packet_batch,
                            &mut shred_map,
                            Arc::clone(&shreds_to_ignore),
                            &mut total_shred_received_count
                        );
                    }
                    // handle shutdown
                    recv(shutdown_receiver) -> _ => {
                        break;
                    }
                }
            }
            info!("Exiting forwarder thread.");
        })
        .unwrap();

    vec![listen_thread, process_thread, report_metrics_thread]
}

/// Broadcasts same packet to multiple recipients
/// Returns Err when unable to receive packets.
fn recv_from_channel_and_send_multiple_dest(
    maybe_packet_batch: Result<PacketBatch, RecvError>,
    deduper: &RwLock<Deduper<2, [u8]>>,
    send_socket: &UdpSocket,
    local_dest_sockets: &[SocketAddr],
    debug_trace_shred: bool,
    metrics: &ShredMetrics,
) -> Result<(), ShredstreamProxyError> {
    let packet_batch = maybe_packet_batch.map_err(ShredstreamProxyError::RecvError)?;
    println!("PacketBatch contains {} packets", packet_batch.len());

    for (i, packet) in packet_batch.iter().enumerate() {
        println!("\nDecoding packet {}:", i);
        println!("  Meta: {:?}", packet.meta());
        println!("  Size: {}", packet.meta().size);
        println!("  Size: {:?}", packet.meta().flags);
        println!("PACKET : {:?}", packet);
        
        let _result: Result<solana_ledger::shred::Shred, solana_ledger::shred::Error> = 
            Shred::new_from_serialized_shred(packet.data(..).unwrap().to_vec());
    }

    let trace_shred_received_time = SystemTime::now();
    metrics
        .agg_received
        .fetch_add(packet_batch.len() as u64, Ordering::Relaxed);
    debug!(
        "Got batch of {} packets, total size in bytes: {}",
        packet_batch.len(),
        packet_batch.iter().map(|x| x.meta().size).sum::<usize>()
    );

    let mut packet_batch_vec = vec![packet_batch];

    let num_deduped = solana_perf::deduper::dedup_packets_and_count_discards(
        &deduper.read().unwrap(),
        &mut packet_batch_vec,
        |_received_packet, _is_already_marked_as_discard, _is_dup| {},
    );

    packet_batch_vec.iter().for_each(|batch| {
        batch.iter().for_each(|packet| {
            metrics
                .packets_received
                .entry(packet.meta().addr)
                .and_modify(|(discarded, not_discarded)| {
                    *discarded += packet.meta().discard() as u64;
                    *not_discarded += !packet.meta().discard() as u64;
                })
                .or_insert_with(|| {
                    (
                        packet.meta().discard() as u64,
                        !packet.meta().discard() as u64,
                    )
                });
        });
    });

    local_dest_sockets.iter().for_each(|outgoing_socketaddr| {
        let packets_with_dest = packet_batch_vec[0].iter().filter_map(|pkt| {
            let data = pkt.data(..)?;
            let addr = outgoing_socketaddr;
            Some((data, addr))
        }).collect::<Vec<(&[u8], &SocketAddr)>>();

        match batch_send(send_socket, &packets_with_dest) {
            Ok(_) => {
                metrics.agg_success_forward.fetch_add(packets_with_dest.len() as u64, Ordering::Relaxed);
                metrics.duplicate.fetch_add(num_deduped, Ordering::Relaxed);
            }
            Err(SendPktsError::IoError(err, num_failed)) => {
                metrics.agg_fail_forward.fetch_add(packets_with_dest.len() as u64, Ordering::Relaxed);
                metrics.duplicate.fetch_add(num_failed as u64, Ordering::Relaxed);
                error!("Failed to send batch of size {} to {outgoing_socketaddr:?}. {num_failed} packets failed. Error: {err}", packets_with_dest.len());
            }
        }
    });

    if debug_trace_shred {
        packet_batch_vec[0]
            .iter()
            .filter_map(|p| TraceShred::decode(p.data(..)?).ok())
            .filter(|t| t.created_at.is_some())
            .for_each(|trace_shred| {
                let elapsed = trace_shred_received_time
                    .duration_since(SystemTime::try_from(trace_shred.created_at.unwrap()).unwrap())
                    .unwrap_or_default();

                datapoint_info!(
                    "shredstream_proxy-trace_shred_latency",
                    "trace_region" => trace_shred.region,
                    ("trace_seq_num", trace_shred.seq_num as i64, i64),
                    ("elapsed_micros", elapsed.as_micros(), i64),
                );
            });
    }
    Ok(())
}

/// Starts a thread that updates our destinations used by the forwarder threads
pub fn start_destination_refresh_thread(
    endpoint_discovery_url: String,
    discovered_endpoints_port: u16,
    static_dest_sockets: Vec<(SocketAddr, String)>,
    unioned_dest_sockets: Arc<ArcSwap<Vec<SocketAddr>>>,
    shutdown_receiver: Receiver<()>,
    exit: Arc<AtomicBool>,
) -> JoinHandle<()> {
    Builder::new().name("ssPxyDstRefresh".to_string()).spawn(move || {
        let fetch_socket_tick = crossbeam_channel::tick(Duration::from_secs(30));
        let metrics_tick = crossbeam_channel::tick(Duration::from_secs(30));
        let mut socket_count = static_dest_sockets.len();
        while !exit.load(Ordering::Relaxed) {
            crossbeam_channel::select! {
                    recv(fetch_socket_tick) -> _ => {
                        let fetched = fetch_unioned_destinations(
                            &endpoint_discovery_url,
                            discovered_endpoints_port,
                            &static_dest_sockets,
                        );
                        let new_sockets = match fetched {
                            Ok(s) => {
                                info!("Sending shreds to {} destinations: {s:?}", s.len());
                                s
                            }
                            Err(e) => {
                                warn!("Failed to fetch from discovery service, retrying. Error: {e}");
                                datapoint_warn!("shredstream_proxy-destination_refresh_error",
                                                ("prev_unioned_dest_count", socket_count, i64),
                                                ("errors", 1, i64),
                                                ("error_str", e.to_string(), String),
                                );
                                continue;
                            }
                        };
                        socket_count = new_sockets.len();
                        unioned_dest_sockets.store(Arc::new(new_sockets));
                    }
                    recv(metrics_tick) -> _ => {
                        datapoint_info!("shredstream_proxy-destination_refresh_stats",
                                        ("destination_count", socket_count, i64),
                        );
                    }
                    recv(shutdown_receiver) -> _ => {
                        break;
                    }
                }
        }
    }).unwrap()
}

/// Returns dynamically discovered endpoints with CLI arg defined endpoints
fn fetch_unioned_destinations(
    endpoint_discovery_url: &str,
    discovered_endpoints_port: u16,
    static_dest_sockets: &[(SocketAddr, String)],
) -> Result<Vec<SocketAddr>, ShredstreamProxyError> {
    let bytes = reqwest::blocking::get(endpoint_discovery_url)?.bytes()?;

    let sockets_json = match serde_json::from_slice::<Vec<IpAddr>>(&bytes) {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "Failed to parse json from: {:?}",
                std::str::from_utf8(&bytes)
            );
            return Err(ShredstreamProxyError::from(e));
        }
    };

    // resolve again since ip address could change
    let static_dest_sockets = static_dest_sockets
        .iter()
        .filter_map(|(_socketaddr, hostname_port)| {
            Some(resolve_hostname_port(hostname_port).ok()?.0)
        })
        .collect::<Vec<_>>();

    let unioned_dest_sockets = sockets_json
        .into_iter()
        .map(|ip| SocketAddr::new(ip, discovered_endpoints_port))
        .chain(static_dest_sockets)
        .unique()
        .collect::<Vec<SocketAddr>>();
    Ok(unioned_dest_sockets)
}

/// Reset dedup + send metrics to influx
pub fn start_forwarder_accessory_thread(
    deduper: Arc<RwLock<Deduper<2, [u8]>>>,
    metrics: Arc<ShredMetrics>,
    metrics_update_interval_ms: u64,
    grpc_restart_signal: Sender<()>,
    shutdown_receiver: Receiver<()>,
    exit: Arc<AtomicBool>,
) -> JoinHandle<()> {
    Builder::new()
        .name("ssPxyAccessory".to_string())
        .spawn(move || {
            let metrics_tick =
                crossbeam_channel::tick(Duration::from_millis(metrics_update_interval_ms));
            let deduper_reset_tick = crossbeam_channel::tick(Duration::from_secs(2));
            let stale_connection_tick = crossbeam_channel::tick(Duration::from_secs(30));
            let mut rng = rand::thread_rng();
            let mut last_cumulative_received_shred_count = 0;
            while !exit.load(Ordering::Relaxed) {
                crossbeam_channel::select! {
                    // reset deduper to avoid false positives
                    recv(deduper_reset_tick) -> _ => {
                        deduper
                            .write()
                            .unwrap()
                            .maybe_reset(&mut rng, DEDUPER_FALSE_POSITIVE_RATE, DEDUPER_RESET_CYCLE);
                    }

                    // send metrics to influx
                    recv(metrics_tick) -> _ => {
                        metrics.report();
                        metrics.reset();
                    }

                    // handle scenario when grpc connection is open, but backend doesn't receive heartbeat
                    // possibly due to envoy losing track of the pod when backend restarts.
                    // we restart our grpc connection to work around the stale connection
                    recv(stale_connection_tick) -> _ => {
                        // if no shreds received, then restart
                        let new_received_count = metrics.agg_received_cumulative.load(Ordering::Relaxed);
                        if new_received_count == last_cumulative_received_shred_count {
                            if let Err(TrySendError::Disconnected(())) = grpc_restart_signal.try_send(()) {
                                panic!("Failed to send grpc restart signal, channel disconnected");
                            }
                        }
                        last_cumulative_received_shred_count = new_received_count;
                    }

                    // handle SIGINT shutdown
                    recv(shutdown_receiver) -> _ => {
                        break;
                    }
                }
            }
        })
        .unwrap()
}

pub struct ShredMetrics {
    /// Total number of shreds received. Includes duplicates when receiving shreds from multiple regions
    pub agg_received: AtomicU64,
    /// Total number of shreds successfully forwarded, accounting for all destinations
    pub agg_success_forward: AtomicU64,
    /// Total number of shreds failed to forward, accounting for all destinations
    pub agg_fail_forward: AtomicU64,
    /// Number of duplicate shreds received
    pub duplicate: AtomicU64,
    /// (discarded, not discarded, from other shredstream instances)
    pub packets_received: DashMap<IpAddr, (u64, u64)>,

    // cumulative metrics (persist after reset)
    pub agg_received_cumulative: AtomicU64,
    pub agg_success_forward_cumulative: AtomicU64,
    pub agg_fail_forward_cumulative: AtomicU64,
    pub duplicate_cumulative: AtomicU64,
}

impl ShredMetrics {
    pub fn new() -> Self {
        Self {
            agg_received: Default::default(),
            agg_success_forward: Default::default(),
            agg_fail_forward: Default::default(),
            duplicate: Default::default(),
            packets_received: DashMap::with_capacity(10),
            agg_received_cumulative: Default::default(),
            agg_success_forward_cumulative: Default::default(),
            agg_fail_forward_cumulative: Default::default(),
            duplicate_cumulative: Default::default(),
        }
    }

    pub fn report(&self) {
        datapoint_info!(
            "shredstream_proxy-connection_metrics",
            (
                "agg_received",
                self.agg_received.load(Ordering::Relaxed),
                i64
            ),
            (
                "agg_success_forward",
                self.agg_success_forward.load(Ordering::Relaxed),
                i64
            ),
            (
                "agg_fail_forward",
                self.agg_fail_forward.load(Ordering::Relaxed),
                i64
            ),
            ("duplicate", self.duplicate.load(Ordering::Relaxed), i64),
        );
        self.packets_received.iter().for_each(|kv| {
            let (addr, (discarded_packets, not_discarded_packets)) = kv.pair();
            datapoint_info!("shredstream_proxy-receiver_stats",
                "addr" => addr.to_string(),
                ("discarded_packets", *discarded_packets, i64),
                ("not_discarded_packets", *not_discarded_packets, i64),
            );
        });
    }

    /// resets current values, increments cumulative values
    pub fn reset(&self) {
        self.agg_received_cumulative.fetch_add(
            self.agg_received.swap(0, Ordering::Relaxed),
            Ordering::Relaxed,
        );
        self.agg_success_forward_cumulative.fetch_add(
            self.agg_success_forward.swap(0, Ordering::Relaxed),
            Ordering::Relaxed,
        );
        self.agg_fail_forward_cumulative.fetch_add(
            self.agg_fail_forward.swap(0, Ordering::Relaxed),
            Ordering::Relaxed,
        );
        self.duplicate_cumulative
            .fetch_add(self.duplicate.swap(0, Ordering::Relaxed), Ordering::Relaxed);
        self.packets_received.alter_all(|_ip, _metrics| (0, 0))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
        str::FromStr,
        sync::{Arc, Mutex, RwLock},
        thread,
        thread::sleep,
        time::Duration,
    };

    use solana_perf::{
        deduper::Deduper,
        packet::{Meta, Packet, PacketBatch},
    };
    use solana_sdk::packet::{PacketFlags, PACKET_DATA_SIZE};

    use crate::forwarder::{recv_from_channel_and_send_multiple_dest, ShredMetrics};

    fn listen_and_collect(listen_socket: UdpSocket, received_packets: Arc<Mutex<Vec<Vec<u8>>>>) {
        let mut buf = [0u8; PACKET_DATA_SIZE];
        loop {
            listen_socket.recv(&mut buf).unwrap();
            received_packets.lock().unwrap().push(Vec::from(buf));
        }
    }

    #[test]
    fn test_2shreds_3destinations() {
        let packet_batch = PacketBatch::new(vec![
            Packet::new(
                [1; PACKET_DATA_SIZE],
                Meta {
                    size: PACKET_DATA_SIZE,
                    addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    port: 48289, // received on random port
                    flags: PacketFlags::empty(),
                },
            ),
            Packet::new(
                [2; PACKET_DATA_SIZE],
                Meta {
                    size: PACKET_DATA_SIZE,
                    addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    port: 9999,
                    flags: PacketFlags::empty(),
                },
            ),
        ]);
        let (packet_sender, packet_receiver) = crossbeam_channel::unbounded::<PacketBatch>();
        packet_sender.send(packet_batch).unwrap();

        let dest_socketaddrs = vec![
            SocketAddr::from_str("0.0.0.0:32881").unwrap(),
            SocketAddr::from_str("0.0.0.0:33881").unwrap(),
            SocketAddr::from_str("0.0.0.0:34881").unwrap(),
        ];

        let test_listeners = dest_socketaddrs
            .iter()
            .map(|socketaddr| {
                (
                    UdpSocket::bind(socketaddr).unwrap(),
                    *socketaddr,
                    // store results in vec of packet, where packet is Vec<u8>
                    Arc::new(Mutex::new(vec![])),
                )
            })
            .collect::<Vec<_>>();

        let udp_sender = UdpSocket::bind("0.0.0.0:10000").unwrap();

        // spawn listeners
        test_listeners
            .iter()
            .for_each(|(listen_socket, _socketaddr, to_receive)| {
                let socket = listen_socket.try_clone().unwrap();
                let to_receive = to_receive.to_owned();
                thread::spawn(move || listen_and_collect(socket, to_receive));
            });

        // send packets
        recv_from_channel_and_send_multiple_dest(
            packet_receiver.recv(),
            &Arc::new(RwLock::new(Deduper::<2, [u8]>::new(
                &mut rand::thread_rng(),
                crate::forwarder::DEDUPER_NUM_BITS,
            ))),
            &udp_sender,
            &Arc::new(dest_socketaddrs),
            false,
            &Arc::new(ShredMetrics::new()),
        )
        .unwrap();

        // allow packets to be received
        sleep(Duration::from_millis(500));

        let received = test_listeners
            .iter()
            .map(|(_, _, results)| results.clone())
            .collect::<Vec<_>>();

        // check results
        for received in received.iter() {
            let received = received.lock().unwrap();
            assert_eq!(received.len(), 2);
            assert!(received
                .iter()
                .all(|packet| packet.len() == PACKET_DATA_SIZE));
            assert_eq!(received[0], [1; PACKET_DATA_SIZE]);
            assert_eq!(received[1], [2; PACKET_DATA_SIZE]);
        }

        assert_eq!(
            received
                .iter()
                .fold(0, |acc, elem| acc + elem.lock().unwrap().len()),
            6
        );
    }
}
