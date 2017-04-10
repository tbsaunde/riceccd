extern crate iceccd;

use iceccd::*;
use iceccd::MsgChannel;
use iceccd::Msg;
use iceccd::MsgType;
use iceccd::send_msg;

extern crate clap;
use clap::{Arg, App};
use std::net::{TcpStream, UdpSocket, ToSocketAddrs};

extern crate get_if_addrs;
extern crate resolve;

extern crate local_socket;

use local_socket::*;
use std::thread;
use std::sync::Mutex;
use std::time::Duration;

extern crate crossbeam;

fn socket_thread(shared_chan: & Mutex<MsgChannel>, local_sock: &str)
{
    println!("scoket thread started");
    let mut socket = LocalListener::bind("/tmp/riceccd2.sock").expect(local_sock);
    for stream in socket.incoming() {
        println!("{:?}", stream.expect("stream"));
    }
}

fn main()
{
    let cmd_args = App::new("riceccd")
        .version("0.0")
        .about("riceccd")
        .author("Trevor Saunders")
        .arg(Arg::with_name("network")
             .short("n")
             .long("network")
             .help("icecream network")
             .takes_value(true))
        .arg(Arg::with_name("socket")
             .short("s")
             .long("socket")
             .help("local socket to listen for connections from clients on")
             .takes_value(true))
        .get_matches();
    let network = cmd_args.value_of("network").unwrap_or("ICECREAM");
    let local_socket: &str = cmd_args.value_of("socket").unwrap_or("/tmp/riceccd.sock");
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
        if net == network {
            break;
        }
    }

    let mut sched_sock = MsgChannel::new(sched.unwrap());
    sched_sock.stream.set_nodelay(true).expect("nodelay");
    // sched_sock.set_nonblocking(true).expect("nonblocking");
    println!("{:#?}", sched_sock.stream);

    let host_name :String = resolve::hostname::get_hostname().expect("hostname");
    println!("{}", host_name);
    let mut login_msg = Msg::new(MsgType::LOGIN);
    login_msg.append_u32(0); // not supporting remote connections so port 0 is fine.
    login_msg.append_u32(8); // not supporting remote connections so this doesn't really matter.
    login_msg.append_u32(0); // no envs.
    login_msg.append_str("cat");
    login_msg.append_str("x86_64");
    login_msg.append_u32(0); // chroot_possible is false.
    login_msg.append_u32(1); // noremote.
    send_msg(&mut sched_sock.stream, &login_msg);

    let mut stats_msg = Msg::new(MsgType::STATS);
    stats_msg.append_u32(0);
    stats_msg.append_u32(0);
    stats_msg.append_u32(0);
    stats_msg.append_u32(0);
    send_msg(&mut sched_sock.stream, &stats_msg);

    let shared_sched: Mutex<MsgChannel> = Mutex::new(sched_sock);
    crossbeam::scope(|scope| {
        scope.spawn(|| { socket_thread(&shared_sched, local_socket) });

loop {
    thread::sleep(Duration::from_secs(10));
    send_msg(&mut shared_sched.lock().unwrap().stream, &stats_msg);
    display_msg(&mut shared_sched.lock().unwrap());
}
});
}
