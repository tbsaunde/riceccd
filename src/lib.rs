use std::net::{TcpStream, UdpSocket, ToSocketAddrs};
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::vec;
use byteorder::{NetworkEndian, ByteOrder};

extern crate get_if_addrs;
extern crate resolve;
extern crate byteorder;
extern crate minilzo;

#[derive(Copy, Clone,Debug)]
pub enum MsgType
{
    End = 67,
    GetCS = 71,
    CompileFile = 73,
    FileChunk = 74,
    Login = 80,
    Stats = 81,
    EnvTransfer = 88,
    VerifyEnv = 93,
}

pub struct Msg
{
    msgtype: MsgType,
    data: Vec<u8>,
}

impl Msg
{
pub     fn new(msgtype: MsgType) -> Msg
    {
        Msg{
            msgtype: msgtype,
            data: Vec::new()
        }
    }

pub     fn append_u32(&mut self, val: u32)
    {
        let mut buf = [0 ; 4 ];
        NetworkEndian::write_u32(&mut buf, val);
        self.data.extend_from_slice(&buf);
    }

pub     fn append_str(&mut self, s: &str)
    {
        self.append_u32((s.len() + 1) as u32);
        self.data.extend_from_slice(s.as_bytes());
        self.data.push(0);
    }

pub     fn append_envs(&mut self, envs: Vec<(&str, &str)>)
    {
        self.append_u32(envs.len() as u32);
        for env in envs {
            self.append_str(env.0);
            self.append_str(env.1);
        }
    }

    pub fn len(&self) -> usize
    {
        self.data.len() + 4
    }
}

pub struct MsgChannel
{
    pub stream :TcpStream,
    pub protocol :u32,
}

impl MsgChannel
{
pub fn new<A: ToSocketAddrs>(addr: A) -> MsgChannel
    {
        let mut sock = TcpStream::connect(addr).expect("connect");
        let protobuf = [35, 0, 0, 0];
        sock.write(&protobuf).expect("write proto");

        // Get the maximum protocol version we have in common with the scheduler or daemon.
        let proto = read_u32le(&mut sock);
        println!("proto version {}", proto);

        let protobuf = [proto as u8, 0, 0, 0];
        sock.write(&protobuf).expect("write proto");

        let proto = read_u32le(&mut sock);
        println!("proto version {}", proto);

        MsgChannel{
            stream: sock,
            protocol: proto
        }
    }
}

pub fn send_msg<W: Write>(mut sock: W, msg: &Msg)
{
    assert!(msg.len() < 1000000);
    let len = [0, (msg.len() >> 16) as u8, (msg.len() >> 8) as u8, msg.len() as u8]; // fix me
    sock.write(&len);
    let typebuf = [0, 0, (msg.msgtype as u16 >> 8) as u8, msg.msgtype as u8];
    sock.write(&typebuf).expect("write type");
    let write_len = sock.write(msg.data.as_slice()).expect("write");
    println!("sent packet type {:?} length {}", msg.msgtype, write_len);
}

fn send_file(sock: &mut TcpStream, path: &str)
{
    let mut f = File::open(path).expect("open");
    let mut buf: Vec<u8> = Vec::new();
    let len = f.read_to_end(&mut buf).expect("read file");
    for chunk in buf.chunks(100000) {
        let mut fcmsg = Msg::new(MsgType::FileChunk);
        fcmsg.append_u32(chunk.len() as u32);
        let mut compressed = minilzo::compress(chunk).expect("compression");
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

pub fn send_compile_file_msg(stream: &mut MsgChannel, job_id :u32)
{
    let mut msg = Msg::new(MsgType::CompileFile);
    msg.append_u32(0); // language of source
    msg.append_u32(job_id);
    msg.append_u32(0); // remote flags
    msg.append_u32(0); // rest flags
    msg.append_str("foo.tar.gz"); // environment
    msg.append_str("x86_64"); // target platform
    msg.append_str("gcc"); // compiler name
    if stream.protocol >= 34 {
        msg.append_str("bar.c"); // input file
        msg.append_str("/tmp/"); // cwd
    }
    if stream.protocol >= 35 {
        msg.append_str("bar.o"); // output file name
        msg.append_u32(0); // dwo enabled
    }

    send_msg(&mut stream.stream, &msg);
}

pub fn get_cs(con: &mut MsgChannel, file: &str, lang: SourceLanguage)
{
    let mut get_cs_msg = Msg::new(MsgType::GetCS);

    // a set of (host platform, tool chain file) pairs for each platform we have a toolchain for.
    // Currently we just hardcode this.
    let envs = vec!(("x86_64", "foo.tar.gz"));
    get_cs_msg.append_envs(envs);

    // information about the file we'll compile to show to things monitoring jobs.
    get_cs_msg.append_str(file);
    get_cs_msg.append_u32(lang as u32);

    // the number of jobs we'd like to run, this is only used by icecream when compiling a file
    // multiple times which we don't do yet.
    get_cs_msg.append_u32(1);

    // the type of platform we would prefer the compile server be.
    get_cs_msg.append_str("x86_64");

    // argument flags, aparently only used in calculating speed of compile servers so unimplemented
    // for now.
    get_cs_msg.append_u32(0);

    // client id is really a daemon id that requested a compile server allocation from the scheduler.
    get_cs_msg.append_u32(53);

    // preferred host is to use a particular daemon, we don't support that yet.
    get_cs_msg.append_str("");
    
    if con.protocol >= 31 {
        get_cs_msg.append_u32(0);
        if con.protocol >= 34 {
            get_cs_msg.append_u32(0);
        }
    }

    send_msg(&mut con.stream, &get_cs_msg);
}

pub fn run_job(host: &str, port: u32, host_platform: &str, job_id: u32, got_env: bool)
{
    let mut cssock = MsgChannel::new((host, port as u16));
    if !got_env {
        let mut env_transfer_msg = Msg::new(MsgType::EnvTransfer);
        env_transfer_msg.append_str("foo.tar.gz");
        env_transfer_msg.append_str(host_platform);
        send_msg(&mut cssock.stream, &env_transfer_msg);
        send_file(&mut cssock.stream, "/tmp/foo.tar.gz");
        send_msg(&mut cssock.stream, &Msg::new(MsgType::End));

        let mut verify_msg = Msg::new(MsgType::VerifyEnv);
        verify_msg.append_str("foo.tar.gz");
        verify_msg.append_str("x86_64");
        send_msg(&mut cssock.stream, &verify_msg);
        display_msg(&mut cssock);
    }

    send_compile_file_msg(&mut cssock, job_id);
    send_file(&mut cssock.stream, "/tmp/bar.c");
    send_msg(&mut cssock.stream, &Msg::new(MsgType::End));
    loop {
        display_msg(&mut cssock);
    }
}

pub fn display_msg(sock: &mut MsgChannel)
{
    let msglen = read_u32be(&mut sock.stream);
    let msgtype = read_u32be(&mut sock.stream);
    println!("msg length {} type {}", msglen, msgtype);
    let mut buf: Vec<u8> = Vec::with_capacity(msglen as usize);
    buf.resize(msglen as usize, 0);
    match msgtype {
        92 => {
            let max_scheduler_pong = read_u32be(&mut sock.stream);
            let max_scheduler_ping = read_u32be(&mut sock.stream);
            let bench_source = read_string(&mut sock.stream);
            println!("max scheduler pong {} max scheduler ping {}", max_scheduler_pong, max_scheduler_ping);
        }
        72 => {
            //let ret = sock.read(buf.as_mut_slice()).expect("read buf");
            //println!("data {:?}", buf);
            let job_id = read_u32be(&mut sock.stream);
            let port = read_u32be(&mut sock.stream);
            let host = read_string(&mut sock.stream);
            let host_platform = read_string(&mut sock.stream);
            let got_env = read_u32be(&mut sock.stream);
            let client_id = read_u32be(&mut sock.stream);
            let matched_job_id = read_u32be(&mut sock.stream);
            println!("job {} assigned to {}:{} platform {} got_env {} for client {} matched {}", job_id, host, port, host_platform, got_env, client_id, matched_job_id);
            run_job(&host, port, &host_platform, job_id, got_env != 0);
        }
        94 => {
            let val = read_u32be(&mut sock.stream);
            println!("verification of env is {}", val);
        }
        75 => {
            let stderr = read_string(&mut sock.stream);
            let stdout = read_string(&mut sock.stream);
            let status = read_u32be(&mut sock.stream);
            let oom = read_u32be(&mut sock.stream);
            println!("compile finished status {} stdout {} stderr {}  oom {}", status, stdout, stderr, oom);
        }
        90 => {
            let str = read_string(&mut sock.stream);
            println!("status text: {}", str);
        }
        67 => {
            println!("end msg");
        }
        i =>  { println!("unmatched type {}", msgtype) }
    }
}

fn main()
{
    let mut sched_sock = get_scheduler(&start_udp_discovery(), "icecc-test").unwrap();
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

    display_msg(&mut sched_sock);

    let mut get_cs_msg = Msg::new(MsgType::GetCS);
    let envs = vec!(("x86_64", "foo.tar.gz"));
    get_cs_msg.append_envs(envs);
    get_cs_msg.append_str("/tmp/test-icecc.c");
    get_cs_msg.append_u32(0);
    get_cs_msg.append_u32(1);
    get_cs_msg.append_str("x86_64");
    get_cs_msg.append_u32(0);
    get_cs_msg.append_u32(53);
    get_cs_msg.append_str("");
    get_cs_msg.append_u32(0);
    send_msg(&mut sched_sock.stream, &get_cs_msg);

    loop {
        display_msg(&mut sched_sock);
    }
}

pub fn start_udp_discovery() -> UdpSocket
{
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
    sock
}

pub fn get_scheduler(sock: & UdpSocket, network: & str) -> Option<MsgChannel>
{
    let mut sched: Option<std::net::SocketAddr>;
    loop {
        let mut ans = [0; 30];
        let (_, s) = sock.recv_from(&mut ans).expect("read");
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

    let sched_sock = MsgChannel::new(sched.unwrap());
    sched_sock.stream.set_nodelay(true).expect("nodelay");
    // sched_sock.set_nonblocking(true).expect("nonblocking");
    println!("{:#?}", sched_sock.stream);
    Some(sched_sock)
}

pub enum SourceLanguage
{
    C = 0,
    CPlusPlus = 1,
    OBJC = 2,
    Custom = 3,
}
