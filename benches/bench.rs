#![allow(dead_code, unused_macros, unused_imports)]
#[macro_use] extern crate serde;
#[macro_use] extern crate log;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

use smallvec::smallvec;
use ring::aead;

use std::str::FromStr;
use std::net::{SocketAddr, Ipv4Addr, SocketAddrV4, UdpSocket};

mod util {
    include!("../src/util.rs");
}
mod error {
    include!("../src/error.rs");
}
mod payload {
    include!("../src/payload.rs");
}
mod types {
    include!("../src/types.rs");
}
mod table {
    include!("../src/table.rs");
}
mod crypto_core {
    include!("../src/crypto/core.rs");
}

pub use error::Error;
use util::{MockTimeSource, MsgBuffer};
use types::{Address, Range};
use table::{ClaimTable};
use payload::{Packet, Frame, Protocol};
use crypto_core::{create_dummy_pair, EXTRA_LEN};

fn udp_send(c: &mut Criterion) {
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    let data = [0; 1400];
    let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1);
    let mut g = c.benchmark_group("udp_send");
    g.throughput(Throughput::Bytes(1400));
    g.bench_function("udp_send", |b| {
        b.iter(|| sock.send_to(&data, &addr).unwrap());
    });
    g.finish();
}

fn decode_ipv4(c: &mut Criterion) {
    let data = [0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 192, 168, 1, 1, 192, 168, 1, 2];
    let mut g = c.benchmark_group("payload");
    g.throughput(Throughput::Bytes(1400));
    g.bench_function("decode_ipv4", |b| {
        b.iter(|| Packet::parse(&data).unwrap());
    });
    g.finish();
}

fn decode_ipv6(c: &mut Criterion) {
    let data = [
        0x60, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 0, 9, 8, 7, 6, 5, 4, 3, 2, 1, 6, 5,
        4, 3, 2, 1
    ];
    let mut g = c.benchmark_group("payload");
    g.throughput(Throughput::Bytes(1400));
    g.bench_function("decode_ipv6", |b| {
        b.iter(|| Packet::parse(&data).unwrap());
    });
    g.finish();
}

fn decode_ethernet(c: &mut Criterion) {
    let data = [6, 5, 4, 3, 2, 1, 1, 2, 3, 4, 5, 6, 1, 2, 3, 4, 5, 6, 7, 8];
    let mut g = c.benchmark_group("payload");
    g.throughput(Throughput::Bytes(1400));
    g.bench_function("decode_ethernet", |b| {
        b.iter(|| Frame::parse(&data).unwrap());
    });
    g.finish();
}

fn decode_ethernet_with_vlan(c: &mut Criterion) {
    let data = [6, 5, 4, 3, 2, 1, 1, 2, 3, 4, 5, 6, 0x81, 0, 4, 210, 1, 2, 3, 4, 5, 6, 7, 8];
    let mut g = c.benchmark_group("payload");
    g.throughput(Throughput::Bytes(1400));
    g.bench_function("decode_ethernet_with_vlan", |b| {
        b.iter(|| Frame::parse(&data).unwrap());
    });
    g.finish();
}

fn lookup_warm(c: &mut Criterion) {
    let mut table = ClaimTable::<MockTimeSource>::new(60, 60);
    let addr = Address::from_str("1.2.3.4").unwrap();
    table.cache(addr, SocketAddr::from_str("1.2.3.4:3210").unwrap());
    let mut g = c.benchmark_group("table");
    g.throughput(Throughput::Bytes(1400));
    g.bench_function("lookup_warm", |b| {
        b.iter(|| table.lookup(addr));
    });
    g.finish();
}

fn lookup_cold(c: &mut Criterion) {
    let mut table = ClaimTable::<MockTimeSource>::new(60, 60);
    let addr = Address::from_str("1.2.3.4").unwrap();
    table.set_claims(SocketAddr::from_str("1.2.3.4:3210").unwrap(), smallvec![Range::from_str("1.2.3.4/32").unwrap()]);
    let mut g = c.benchmark_group("table");
    g.throughput(Throughput::Bytes(1400));
    g.bench_function("lookup_cold", |b| {
        b.iter(|| {
            table.clear_cache();
            table.lookup(addr)
        });
    });
    g.finish();
}

fn crypto_bench(c: &mut Criterion, algo: &'static aead::Algorithm) {
    let mut buffer = MsgBuffer::new(EXTRA_LEN);
    buffer.set_length(1400);
    let (mut sender, mut receiver) = create_dummy_pair(algo);
    let mut g = c.benchmark_group("crypto");
    g.throughput(Throughput::Bytes(2*1400));
    g.bench_function(format!("{:?}", algo), |b| {
        b.iter(|| {
            sender.encrypt(&mut buffer);
            receiver.decrypt(&mut buffer).unwrap();
        });
    });
    g.finish()
}

fn crypto_chacha20(c: &mut Criterion) {
    crypto_bench(c, &aead::CHACHA20_POLY1305)
}

fn crypto_aes128(c: &mut Criterion) {
    crypto_bench(c, &aead::AES_128_GCM)
}

fn crypto_aes256(c: &mut Criterion) {
    crypto_bench(c, &aead::AES_256_GCM)
}

criterion_group!(benches, udp_send, decode_ipv4, decode_ipv6, decode_ethernet, decode_ethernet_with_vlan, lookup_cold, lookup_warm, crypto_chacha20, crypto_aes128, crypto_aes256);
criterion_main!(benches);