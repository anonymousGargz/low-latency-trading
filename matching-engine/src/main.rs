use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::UdpSocket;
use std::time::Instant;

use hdrhistogram::Histogram;

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub qty: u32,
}

pub struct OrderBook {
    pub bids: BTreeMap<Reverse<i64>, VecDeque<Order>>, // highest price first
    pub asks: BTreeMap<i64, VecDeque<Order>>,          // lowest price first
    pub order_index: HashMap<u64, (i64, u8)>,          // order_id -> (price, side)
}

impl OrderBook {
    pub fn new() -> Self {
        OrderBook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_index: HashMap::new(),
        }
    }

    pub fn add_order(&mut self, id: u64, price: i64, mut qty: u32, side: u8) {
        if side == 0 {
            // BUY order: match against asks (side == 1)
            while qty > 0 {
                let best = match self.asks.iter_mut().next() {
                    Some((&p, _)) if p <= price => p,
                    _ => break, // no ask cheap enough to match
                };

                let queue = self.asks.get_mut(&best).unwrap();
                let traded_qty;
                let resting_id;
                let mut remove_order_id = None;

                {
                    let resting = queue.front_mut().unwrap();
                    resting_id = resting.id;
                    traded_qty = qty.min(resting.qty);
                    resting.qty -= traded_qty;
                    if resting.qty == 0 {
                        remove_order_id = Some(resting.id);
                    }
                }

                qty -= traded_qty;

                println!(
                    "trade: buy_id={} sell_id={} price={} qty={}",
                    id, resting_id, best, traded_qty
                );

                if let Some(filled_id) = remove_order_id {
                    queue.pop_front();
                    self.order_index.remove(&filled_id);
                    if queue.is_empty() {
                        self.asks.remove(&best);
                    }
                }
            }

            if qty > 0 {
                self.bids.entry(Reverse(price)).or_insert_with(VecDeque::new)
                    .push_back(Order { id, qty });
                self.order_index.insert(id, (price, side));
            }

        } else {
            // SELL order: match against bids (side == 0)
            while qty > 0 {
                let best = match self.bids.iter_mut().next() {
                    Some((&Reverse(p), _)) if p >= price => p,
                    _ => break, // no bid high enough to match
                };

                let queue = self.bids.get_mut(&Reverse(best)).unwrap();
                let traded_qty;
                let resting_id;
                let mut remove_order_id = None;

                {
                    let resting = queue.front_mut().unwrap();
                    resting_id = resting.id;
                    traded_qty = qty.min(resting.qty);
                    resting.qty -= traded_qty;
                    if resting.qty == 0 {
                        remove_order_id = Some(resting.id);
                    }
                }

                qty -= traded_qty;

                println!(
                    "trade: sell_id={} buy_id={} price={} qty={}",
                    id, resting_id, best, traded_qty
                );

                if let Some(filled_id) = remove_order_id {
                    queue.pop_front();
                    self.order_index.remove(&filled_id);
                    if queue.is_empty() {
                        self.bids.remove(&Reverse(best));
                    }
                }
            }

            if qty > 0 {
                self.asks.entry(price).or_insert_with(VecDeque::new)
                    .push_back(Order { id, qty });
                self.order_index.insert(id, (price, side));
            }
        }
    }

    pub fn cancel_order(&mut self, id: u64) {
        if let Some((price, side)) = self.order_index.remove(&id) {
            if side == 0 {
                if let Some(queue) = self.bids.get_mut(&Reverse(price)) {
                    queue.retain(|o| o.id != id);
                    if queue.is_empty() {
                        self.bids.remove(&Reverse(price));
                    }
                }
            } else {
                if let Some(queue) = self.asks.get_mut(&price) {
                    queue.retain(|o| o.id != id);
                    if queue.is_empty() {
                        self.asks.remove(&price);
                    }
                }
            }
        }
    }
}

//Define the wire format(simple binary UDP message format for order events)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OrderMsg {
    pub order_id: u64,
    pub price: i64,      // fixed-point, e.g. price * 10000
    pub qty: u32,
    pub side: u8,        // 0 = buy, 1 = sell
    pub kind: u8,        // 0 = new, 1 = cancel
    pub _pad: [u8; 2],
}

fn parse_order_msg(buf: &[u8]) -> OrderMsg {
    OrderMsg {
        order_id: u64::from_le_bytes(buf[0..8].try_into().unwrap()),
        price: i64::from_le_bytes(buf[8..16].try_into().unwrap()),
        qty: u32::from_le_bytes(buf[16..20].try_into().unwrap()),
        side: buf[20],
        kind: buf[21],
        _pad: [buf[22], buf[23]],
    }
}

//NAIIVE UDP RECIEVER
fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:9000")?;
    let mut buf = [0u8; 1024];
    let mut book = OrderBook::new();
    let mut hist = Histogram::<u64>::new_with_bounds(1, 1_000_000, 3).unwrap();
    hist.auto(true);
    let mut count: u64 = 0;

    loop {
        let (len, _src) = socket.recv_from(&mut buf)?;
        let t0 = Instant::now();
        let msg = parse_order_msg(&buf[..len]);
        println!(
            "order_id={} side={} price={} qty={}",
            msg.order_id, msg.side, msg.price, msg.qty
        );
        book.add_order(msg.order_id, msg.price, msg.qty, msg.side);
        let elapsed_ns = t0.elapsed().as_nanos() as u64;
        hist.record(elapsed_ns).unwrap();
        count += 1;

        if count % 10_000 == 0 {
            println!(
                "latency ns: p50={} p95={} p99={} p99.9={} max={} (n={})",
                hist.value_at_percentile(50.0),
                hist.value_at_percentile(95.0),
                hist.value_at_percentile(99.0),
                hist.value_at_percentile(99.9),
                hist.max(),
                count,
            );
        }
    }
}
