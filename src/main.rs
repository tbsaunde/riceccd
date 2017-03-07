use std::net::{TcpStream, UdpSocket};
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::vec;
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

extern crate get_if_addrs;
extern crate resolve;
extern crate byteorder;
extern crate minilzo;

struct Msg
{
msgtype: u32,
data: Vec<u8>,
}

impl Msg
{
fn new(msgtype: u32) -> Msg
{
Msg{
msgtype: msgtype,
data: Vec::new()
}
}

fn append_u32(&mut self, val: u32)
{
self.data.write_u32::<NetworkEndian>(val);
}

fn append_str(&mut self, s: &str)
{
self.append_u32((s.len() + 1) as u32);
self.data.write(s.as_bytes());
self.data.push(0);
}

fn append_envs(&mut self, envs: Vec<(&str, &str)>)
{
self.append_u32(envs.len() as u32);
for env in envs {
self.append_str(env.0);
self.append_str(env.1);
}
}

fn len(&self) -> usize
{
self.data.len() + 4
}
}

fn send_msg(sock: &mut TcpStream, msg: &Msg)
{
assert!(msg.len() < 1000000);
let len = [0, (msg.len() >> 16) as u8, (msg.len() >> 8) as u8, msg.len() as u8]; // fix me
sock.write(&len);
let typebuf = [0, 0, 0, msg.msgtype as u8];
sock.write(&typebuf).expect("write type");
let write_len = sock.write(msg.data.as_slice()).expect("write");
println!("sent packet {}", write_len);
}

fn send_file(sock: &mut TcpStream, path: &str)
{
let mut f = File::open(path).expect("open");
let mut buf: Vec<u8> = Vec::new();
let len = f.read_to_end(&mut buf).expect("read file");
for chunk in buf.chunks(100000) {
let mut fcmsg = Msg::new(74);
fcmsg.append_u32(chunk.len() as u32);
let mut compressed = minilzo::compress(buf.as_slice()).expect("compression");
fcmsg.append_u32(compressed.len() as u32);
fcmsg.data.append(&mut compressed);
send_msg(sock, &fcmsg);
}
}

fn read_u32le(sock: &mut TcpStream) -> u32
{
let mut buf = [0; 4];
let ret = sock.read(&mut buf).expect("read 4");
assert!(ret == 4);
let val: u32 = buf[0] as u32 | (buf[1] as u32) << 8 | (buf[2] as u32) << 16 | (buf[3] as u32) << 24;
val
}

fn read_u32be(sock: &mut TcpStream) -> u32
{
let mut buf = [0; 4];
let ret = sock.read(&mut buf).expect("read 4");
assert!(ret == 4);
let val: u32 = buf[3] as u32 | (buf[2] as u32) << 8 | (buf[1] as u32) << 16 | (buf[0] as u32) << 24;
val
}

fn read_string(sock: &mut TcpStream) -> String
{
let len = read_u32be(sock) as usize;
let mut buf: Vec<u8> = Vec::with_capacity(len);
buf.resize(len, 0);
let mut read :usize = 0;
while read < len {
let mut buf_slice = &mut buf[read..len];
read += sock.read(buf_slice).expect("read string");
}
buf.pop();
String::from_utf8(buf).expect("parse utf8")
}

fn run_job(host: &str, port: u32, host_platform: &str, job_id: u32, got_env: bool)
{
let mut cssock = TcpStream::connect((host, port as u16)).expect("connect");
if !got_env {
let mut env_transfer_msg = Msg::new(88);
env_transfer_msg.append_str("/tmp/foo.tar.gz");
env_transfer_msg.append_str(host_platform);
send_msg(&mut cssock, &env_transfer_msg);
send_file(&mut cssock, "/tmp/foo.tar.gz");
send_msg(&mut cssock, &Msg::new(67));

let mut verify_msg = Msg::new(93);
verify_msg.append_str("/tmp/foo.tar.gz");
verify_msg.append_str("x86_64");
send_msg(&mut cssock, &verify_msg);
display_msg(&mut cssock);
}
}

fn display_msg(sock: &mut TcpStream)
{
let msglen = read_u32be(sock);
let msgtype = read_u32be(sock);
println!("msg length {} type {}", msglen, msgtype);
let mut buf: Vec<u8> = Vec::with_capacity(msglen as usize);
buf.resize(msglen as usize, 0);
match msgtype {
92 => {
let max_scheduler_pong = read_u32be(sock);
let max_scheduler_ping = read_u32be(sock);
let bench_source = read_string(sock);
println!("max scheduler pong {} max scheduler ping {} bench_source \"{}\"", max_scheduler_pong, max_scheduler_ping, bench_source);
}
72 => {
//let ret = sock.read(buf.as_mut_slice()).expect("read buf");
//println!("data {:?}", buf);
let job_id = read_u32be(sock);
let port = read_u32be(sock);
let host = read_string(sock);
let host_platform = read_string(sock);
let got_env = read_u32be(sock);
let client_id = read_u32be(sock);
let matched_job_id = read_u32be(sock);
println!("job {} assigned to {}:{} platform {} got_env {} for client {} matched {}", job_id, host, port, host_platform, got_env, client_id, matched_job_id);
run_job(&host, port, &host_platform, job_id, got_env != 0);
}
	i =>  { println!("unmatched") }
}
}

fn main() {
	let ifaces = get_if_addrs::get_if_addrs().expect("qux");
let sock = UdpSocket::bind("0.0.0.0:0").expect("error");
sock.set_broadcast(true).expect("broadcast");
	for iface in &ifaces {
if iface.is_loopback() {
continue;
}

		match iface.addr {
			get_if_addrs::IfAddr::V4(ref addr) => {
				match addr.broadcast {
					Some(ip) => {
						let buf = [35];
						sock.send_to(&buf, (ip, 8765)).expect("foobar");
}
_ => ()
}
}
				_ => ()
		};
	}

println!("sent packet");
let mut sched: Option<std::net::SocketAddr> = None;
loop {
let mut ans = [0; 30];
let (amt, s) = sock.recv_from(&mut ans).expect("read");
let mut net : String = String::new();
for x in &ans {
if *x != 0 {
	net.push(*x as char);
}
}

net .remove(0);
println!("{} {}", net, net.len());
sched = Some(s);
if net.contains("icecc-test") {
break;
}
}

let mut sched_sock = TcpStream::connect(sched.unwrap()).expect("tcp");
sched_sock.set_nodelay(true).expect("nodelay");
// sched_sock.set_nonblocking(true).expect("nonblocking");
println!("{:#?}", sched_sock);

let protobuf = [35, 0, 0, 0];
sched_sock.write(&protobuf).expect("write proto");

// Get the maximum protocol version we have in common with the scheduler.
let proto = read_u32le(&mut sched_sock);
println!("proto version {}", proto);

let protobuf = [proto as u8, 0, 0, 0];
sched_sock.write(&protobuf).expect("write proto");

let proto = read_u32le(&mut sched_sock);
println!("proto version {}", proto);

let host_name :String = resolve::hostname::get_hostname().expect("hostname");
println!("{}", host_name);
let mut login_msg = Msg::new(80);
login_msg.append_u32(0); // not supporting remote connections so port 0 is fine.
login_msg.append_u32(8); // not supporting remote connections so this doesn't really matter.
login_msg.append_u32(0); // no envs.
login_msg.append_str("cat");
login_msg.append_str("x86_64");
login_msg.append_u32(0); // chroot_possible is false.
login_msg.append_u32(1); // noremote.
send_msg(&mut sched_sock, &login_msg);

let mut stats_msg = Msg::new(81);
stats_msg.append_u32(0);
stats_msg.append_u32(0);
stats_msg.append_u32(0);
stats_msg.append_u32(0);
send_msg(&mut sched_sock, &stats_msg);

display_msg(&mut sched_sock);

let mut get_cs_msg = Msg::new(71);
let envs = vec!(("x86_64", "/tmp/foo.tar.gz"));
get_cs_msg.append_envs(envs);
get_cs_msg.append_str("/tmp/test-icecc.c");
get_cs_msg.append_u32(0);
get_cs_msg.append_u32(1);
get_cs_msg.append_str("x86_64");
get_cs_msg.append_u32(0);
get_cs_msg.append_u32(53);
get_cs_msg.append_str("");
get_cs_msg.append_u32(0);
send_msg(&mut sched_sock, &get_cs_msg);

loop {
display_msg(&mut sched_sock);
}
}
