use std::net::{TcpStream, UdpSocket};
use std::io::Read;
use std::io::Write;
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

extern crate get_if_addrs;
extern crate resolve;
extern crate byteorder;


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
if net == "ICECREAM" {
break;
}
}

let mut sched_sock = TcpStream::connect(sched.unwrap()).expect("tcp");
sched_sock.set_nodelay(true).expect("nodelay");
// sched_sock.set_nonblocking(true).expect("nonblocking");
println!("{:#?}", sched_sock);

let host_name :String = resolve::hostname::get_hostname().expect("hostname");
println!("{}", host_name);
let mut login_msg: Vec<u8> = Vec::new();
login_msg.write_u32::<NetworkEndian>(10); // LoginMsg is type 10.
login_msg.write_u32::<NetworkEndian>(0); // not supporting remote connections so port 0 is fine.
login_msg.write_u32::<NetworkEndian>(8); // not supporting remote connections so this doesn't really matter.
login_msg.write_u32::<NetworkEndian>(0); // no envs.
login_msg.write_u32::<NetworkEndian>(4); // hack our node name is 3 + \0.
let nodename = ['c' as u8, 'a' as u8, 't' as u8, 0];
login_msg.write(&nodename);
login_msg.write_u32::<NetworkEndian>(7); // hack our host type is 5 + \0
let host_platform = ['x' as u8, '8' as u8, '6' as u8, '_' as u8, '6' as u8, '4' as u8, 0];
login_msg.write(&host_platform);
login_msg.write_u32::<NetworkEndian>(0); // chroot_possible is false.
login_msg.write_u32::<NetworkEndian>(1); // noremote.
let write_len = sched_sock.write(login_msg.as_slice()).expect("write");
println!("sent packet {}", write_len);
let mut listen_buf = [0; 30];
let len = sched_sock.read(&mut listen_buf).expect("read");
println!("{} {:?} {}", len, listen_buf, listen_buf[0] as char);
}
