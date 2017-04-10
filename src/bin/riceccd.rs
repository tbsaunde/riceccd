extern crate iceccd;

use iceccd::*;

extern crate clap;
use clap::{Arg, App};

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
    run_daemon(network, local_socket);
}

fn run_daemon(network: & str, local_socket: & str)
{
    let mut sched_sock: MsgChannel = get_scheduler(&start_udp_discovery(), network).unwrap();

    let host_name :String = resolve::hostname::get_hostname().expect("hostname");
    println!("{}", host_name);
    let mut login_msg = Msg::new(MsgType::Login);
    login_msg.append_u32(0); // not supporting remote connections so port 0 is fine.
    login_msg.append_u32(8); // not supporting remote connections so this doesn't really matter.
    login_msg.append_u32(0); // no envs.
    login_msg.append_str("cat");
    login_msg.append_str("x86_64");
    login_msg.append_u32(0); // chroot_possible is false.
    login_msg.append_u32(1); // noremote.
    send_msg(&mut sched_sock.stream, &login_msg);

    let mut stats_msg = Msg::new(MsgType::Stats);
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
