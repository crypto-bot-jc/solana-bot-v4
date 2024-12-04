use std::net::{UdpSocket, SocketAddr};
use std::time::Duration;


fn main() -> std::io::Result<()> {
    let address: SocketAddr = "127.0.0.1:2002".parse().unwrap();
    let socket = UdpSocket::bind(address)?;

    // Set socket to non-blocking mode
    socket.set_nonblocking(true)?;

    let mut buf = [0u8; 1024];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((amt, src)) => {
                println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);
            }


            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available yet
                println!("No data yet...");
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
