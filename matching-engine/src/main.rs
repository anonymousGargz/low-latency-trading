use std::net::UdpSocket;
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::cmp::Reverse;

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
                let mut remove_order_id = None;

                {
                    let resting = queue.front_mut().unwrap();
                    traded_qty = qty.min(resting.qty);
                    resting.qty -= traded_qty;
                    if resting.qty == 0 {
                        remove_order_id = Some(resting.id);
                    }
                }

                qty -= traded_qty;

                if let Some(filled_id) = remove_order_id {
                    queue.pop_front();
                    self.order_index.remove(&filled_id);
                    if queue.is_empty() {
                        self.asks.remove(&best);
                    }
                }

                // TODO: emit a "trade executed" event here (id vs filled_id, price=best, qty=traded_qty)
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
                let mut remove_order_id = None;

                {
                    let resting = queue.front_mut().unwrap();
                    traded_qty = qty.min(resting.qty);
                    resting.qty -= traded_qty;
                    if resting.qty == 0 {
                        remove_order_id = Some(resting.id);
                    }
                }

                qty -= traded_qty;

                if let Some(filled_id) = remove_order_id {
                    queue.pop_front();
                    self.order_index.remove(&filled_id);
                    if queue.is_empty() {
                        self.bids.remove(&Reverse(best));
                    }
                }

                // TODO: emit a "trade executed" event here
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

pub struct OrderBook {
    pub bids: BTreeMap<i64, VecDeque<Order>>, // Dictionaary that stays sorted by key automatically; highest price first (reverse iteration)
    pub asks: BTreeMap<i64, VecDeque<Order>>, // lowest price first
    pub order_index: HashMap<u64, (i64, u8)>, // O(1) lookup- order_id -> (price, side), for cancels
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub qty: u32,
}

//NAIIVE UDP RECIEVER
fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:9000")?;
    let mut buf = [0u8; 1024];

    loop {
        let (len, _src) = socket.recv_from(&mut buf)?;
        let msg = parse_order_msg(&buf[..len]);
        // hand off to order book
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