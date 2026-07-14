use std::net::UdpSocket;
use std::time::{Duration, Instant};
use std::thread::sleep;

use hdrhistogram::Histogram;

const NUM_ORDERS: u64 = 100_000;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0")?; // 0 = pick any free port for sending
    let target = "127.0.0.1:9000"; // must match matching-engine's bind address

    let mut hist = Histogram::<u64>::new_with_bounds(1, 1_000_000, 3).unwrap();
    hist.auto(true);

    for order_id in 1..=NUM_ORDERS {
        let price: i64 = 100 + (order_id as i64 % 10); // orders clustered around price 100-109
        let qty: u32 = 10;
        let side: u8 = (order_id % 2) as u8; // alternate buy/sell

        let msg = encode_order_msg(order_id, price, qty, side);

        let t0 = Instant::now();
        socket.send_to(&msg, target)?;
        let elapsed_ns = t0.elapsed().as_nanos() as u64;
        hist.record(elapsed_ns).unwrap();

        sleep(Duration::from_micros(100)); // control send rate
    }

    println!("Sent {} orders. Done.", NUM_ORDERS);
    println!("send_to latency:");
    println!("p50:   {} ns", hist.value_at_quantile(0.50));
    println!("p90:   {} ns", hist.value_at_quantile(0.90));
    println!("p99:   {} ns", hist.value_at_quantile(0.99));
    println!("p99.9: {} ns", hist.value_at_quantile(0.999));
    println!("max:   {} ns", hist.max());

    Ok(())
}

fn encode_order_msg(id: u64, price: i64, qty: u32, side: u8) -> [u8; 24] {
    let mut buf = [0u8; 24];
    buf[0..8].copy_from_slice(&id.to_le_bytes());
    buf[8..16].copy_from_slice(&price.to_le_bytes());
    buf[16..20].copy_from_slice(&qty.to_le_bytes());
    buf[20] = side;
    buf[21] = 0; // kind: 0 = new order
    buf
}