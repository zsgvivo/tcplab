/// 这是等待你完成的代码。正常情况下，本文件是你唯一需要改动的文件。
/// 你可以任意地改动此文件，改动的范围当然不限于已有的五个函数里。（只要已有函数的签名别改，要是签名改了main里面就调用不到了）
/// 在开始写代码之前，请先仔细阅读此文件和api文件。这个文件里的五个函数是等你去完成的，而api里的函数是供你调用的。
/// 提示：TCP是有状态的协议，因此你大概率，会需要一个什么样的数据结构来记录和维护所有连接的状态
use crate::api::{
    app_connected, app_peer_fin, app_peer_rst, app_recv, release_connection, tcp_tx,
    ConnectionIdentifier,
};
use etherparse::{ip_number, Ipv4Header, TcpHeader, TcpHeaderSlice};
use lazy_static::lazy_static;
use pnet::packet::tcp::TcpFlags::FIN;
use pnet::packet::{tcp, Packet};
use std::cmp::min;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;

// const CLOSED: usize = 0;
const SYNSENT: usize = 1;
const ESTABLISHED: usize = 2;
const FINWAIT1: usize = 3;
const FINWAIT2: usize = 4;
const TIMEWAIT: usize = 5;

lazy_static! {
    // 存储 TCP 连接状态的全局变量, 用 Mutex 保护
    // state: tcp 连接当前在 FSM 图中的位置
    // seq: 下一个发送的 tcp 报文序号数
    // ack: 发送的 ack 序号数
    // acked: 未被确认的最小序号数
    static ref TCPSTATE:Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());
    static ref CACHE:Mutex<HashMap<u32, Vec<u8>>> = Mutex::new(HashMap::new());
    static ref CONNECTION:Mutex<ConnectionIdentifier> = Mutex::new(ConnectionIdentifier {
        src: crate::api::IpAndPort { ip: String::from("0.0.0.0"), port: 0 },
        dst: crate::api::IpAndPort { ip: String::from("0.0.0.0"), port: 0 },
    });
}

/// 当有应用想要发起一个新的连接时，会调用此函数。想要连接的对象在conn里提供了。
/// 你应该向想要连接的对象发送SYN报文，执行三次握手的逻辑。
/// 当连接建立好后，你需要调用app_connected函数，通知应用层连接已经被建立好了。
/// param: conn: 连接对象
pub fn app_connect(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    let mut buf = [0u8; 1500];
    // 构造 TCP SYN 报文
    let mut syn = TcpHeader::new(conn.src.port, conn.dst.port, 0, 20000);
    syn.syn = true;
    set_checksum(&mut syn, conn, &[]);

    // 转化并发送报文字节流
    let mut tcp_header_buf = &mut buf[..syn.header_len() as usize];
    syn.write(&mut tcp_header_buf).unwrap();
    tcp_tx(conn, &buf[..syn.header_len() as usize]);

    // 初始化更新 TCP 连接状态
    let mut tcpstate = TCPSTATE.lock().unwrap();
    tcpstate.insert(String::from("state"), SYNSENT);
    tcpstate.insert(String::from("seq"), 1);
    tcpstate.insert(String::from("ack"), 0);

    let mut connection = CONNECTION.lock().unwrap();
    connection.src = conn.src.clone();
    connection.dst = conn.dst.clone();


    let mut vec = Vec::new();
    for i in 0..syn.header_len() as usize {
        vec.push(buf[i]);
    }
    let mut cache = CACHE.lock().unwrap();
    cache.insert(syn.sequence_number, vec);

    // println!("app_connect, {:?}", conn);
}

/// 当应用层想要在一个已经建立好的连接上发送数据时，会调用此函数。
/// param: conn: 连接对象
///        bytes: 数据内容，是字节数组
pub fn app_send(conn: &ConnectionIdentifier, bytes: &[u8]) {
    // TODO 请实现此函数
    // 这里应用层数据 bytes 的长度可能超过 MTU, 需要进行分片, 否则会 panic
    const MTU: usize = 1460; // TCP 报文最大长度, 取典型值1460
    let n = bytes.len() / MTU + 1;
    for i in 0..n {
        let payload = &bytes[i * MTU..min((i + 1) * MTU, bytes.len())];
        let mut tcp_state = TCPSTATE.lock().unwrap();
        // 构造带 payload 的tcp 数据报
        let mut tcp_header = TcpHeader::new(
            conn.src.port,
            conn.dst.port,
            tcp_state.get("seq").unwrap().clone() as u32,
            20000,
        );
        set_checksum(&mut tcp_header, conn, payload);

        let mut buf = [0u8; 1500];
        let buf_len = buf.len();
        let mut unwritten = &mut buf[..];
        tcp_header.write(&mut unwritten).unwrap();
        let tcp_header_ends_at = buf_len - unwritten.len();
        buf[tcp_header_ends_at..tcp_header_ends_at + payload.len()].copy_from_slice(payload);
        tcp_tx(conn, &buf[..tcp_header_ends_at + payload.len()]);

        // 更新 tcp 连接状态
        tcp_state.insert(
            String::from("seq"),
            tcp_header.sequence_number as usize + payload.len(),
        );

        let mut vec = Vec::new();
        for i in 0..tcp_header_ends_at + payload.len() {
            vec.push(buf[i]);
        }
        let mut cache = CACHE.lock().unwrap();
        cache.insert(tcp_header.sequence_number, vec);
    }

    // println!("app_send, {:?}, {:?}", conn, std::str::from_utf8(bytes));
}

/// 当应用层想要半关闭连接(FIN)时，会调用此函数。
/// param: conn: 连接对象
pub fn app_fin(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    // 构造 TCP FIN 报文
    let mut tcp_state = TCPSTATE.lock().unwrap();
    let mut fin = TcpHeader::new(
        conn.src.port,
        conn.dst.port,
        tcp_state.get("seq").unwrap().clone() as u32,
        20000,
    );
    fin.fin = true;
    set_checksum(&mut fin, conn, &[]);
    // 转化并发送报文字节流
    let mut buf = [0u8; 1500];
    let buf_len = buf.len();
    let mut unwritten = &mut buf[..];
    fin.write(&mut unwritten).unwrap();
    let tcp_header_ends_at = buf_len - unwritten.len();
    tcp_tx(conn, &buf[..tcp_header_ends_at]);
    // 更新 tcp 连接状态
    tcp_state.insert(String::from("state"), FINWAIT1);

    // println!("app_fin, {:?}", conn);
}

/// 当应用层想要重置连接(RES)时，会调用此函数
/// param: conn: 连接对象
pub fn app_rst(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    // 构造 TCP RST 报文
    let tcp_state = TCPSTATE.lock().unwrap();
    let mut rst = TcpHeader::new(
        conn.src.port,
        conn.dst.port,
        tcp_state.get("seq").unwrap().clone() as u32,
        20000,
    );
    rst.rst = true;
    set_checksum(&mut rst, conn, &[]);
    // 转化并发送报文字节流
    let mut buf = [0u8; 1500];
    let buf_len = buf.len();
    let mut unwritten = &mut buf[..];
    rst.write(&mut unwritten).unwrap();
    let tcp_header_ends_at = buf_len - unwritten.len();
    tcp_tx(conn, &buf[..tcp_header_ends_at]);
    // println!("app_rst, {:?}", conn);
}

/// 当收到TCP报文时，会调用此函数。
/// 正常情况下，你会对TCP报文，根据报文内容和连接的当前状态加以处理，然后调用0个~多个api文件中的函数
/// param: conn: 连接对象
///        bytes: TCP报文内容，是字节数组。（含TCP报头，不含IP报头）
pub fn tcp_rx(conn: &ConnectionIdentifier, bytes: &[u8]) {
    // TODO 请实现此函数
    // 处理收到的 tcp 报文
    let (tcp_header, payload) = TcpHeader::from_slice(bytes).unwrap();
    // let tcp_packet = tcp::TcpPacket::new(bytes).unwrap();
    // let payload = tcp_packet.payload();

    // println!("rx tcp_header: {:?}", tcp_header);
    // 如果收到 SYNACK 报文
    if tcp_header.syn == true && tcp_header.ack == true {
        // 构造 ACK 报文头
        let mut tcpstate = TCPSTATE.lock().unwrap();
        let mut buf = [0u8; 1500];
        let mut ack = TcpHeader::new(
            conn.src.port,
            conn.dst.port,
            tcp_header.acknowledgment_number,
            20000,
        );
        ack.ack = true;
        ack.acknowledgment_number = tcp_header.sequence_number + 1;
        set_checksum(&mut ack, conn, &[]);

        // 转化成字节流, 调用 tcp_tx 函数发送
        let mut tcp_header_buf = &mut buf[..ack.header_len() as usize];
        ack.write(&mut tcp_header_buf).unwrap();
        tcp_tx(conn, &buf[..ack.header_len() as usize]);

        // 更新 tcp 连接状态
        tcpstate.insert(String::from("state"), ESTABLISHED);
        tcpstate.insert(String::from("ack"), ack.acknowledgment_number as usize);
        tcpstate.insert(String::from("seq"), ack.sequence_number as usize);
        tcpstate.insert(String::from("acked"), tcp_header.acknowledgment_number as usize);


        // 调用app_connected函数，通知应用层连接已经被建立好了
        app_connected(conn);
    }
    // 如果收到普通 ACK
    else if tcp_header.ack == true {
        // 更新 tcp 连接状态
        let mut tcpstate = TCPSTATE.lock().unwrap();
        tcpstate.insert(
            String::from("acked"),
            tcp_header.acknowledgment_number as usize,
        );

        // 如果是 FIN ACK, 更新状态
        if tcpstate.get("state").unwrap().clone() == FINWAIT1 {
            tcpstate.insert(String::from("state"), FINWAIT2);
        }
    }
    // 如果收到对方发来的 FIN 报文
    if tcp_header.fin == true {
        // 调用 app_peer_fin 函数，通知应用层对端半关闭连接
        app_peer_fin(conn);

        // 回复 ACK 报文
        let mut buf = [0u8; 1500];
        let mut ack = TcpHeader::new(
            conn.src.port,
            conn.dst.port,
            tcp_header.acknowledgment_number,
            20000,
        );
        ack.ack = true;
        ack.acknowledgment_number = tcp_header.sequence_number + 1;
        set_checksum(&mut ack, conn, &[]);

        // 转化成字节流, 调用 tcp_tx 函数发送
        let mut tcp_header_buf = &mut buf[..ack.header_len() as usize];
        ack.write(&mut tcp_header_buf).unwrap();
        tcp_tx(conn, &buf[..ack.header_len() as usize]);

        // 更新 tcp 连接状态并通知应用层
        let mut tcpstate = TCPSTATE.lock().unwrap();
        if tcpstate.get("state").unwrap().clone() == FINWAIT2 {
            tcpstate.insert(String::from("state"), TIMEWAIT);
            // 完成四次挥手, 通知 driver 释放连接
            release_connection(conn);
        }
    }
    // 如果收到对方发来的 RST 报文
    if tcp_header.rst == true {
        // 通知应用层
        app_peer_rst(conn);
    }
    // 如果有 payload 数据
    if payload.len() > 0 {
        let mut tcpstate = TCPSTATE.lock().unwrap();
        // 调用 app_recv 函数，通知应用层收到了数据
        app_recv(conn, payload);
        // 回复 ACK 报文
        let mut buf = [0u8; 1500];
        let buf_len = buf.len();
        let mut ack = TcpHeader::new(
            conn.src.port,
            conn.dst.port,
            tcpstate.get("seq").unwrap().clone() as u32,
            20000,
        );
        ack.ack = true;
        ack.acknowledgment_number = tcp_header.sequence_number + payload.len() as u32;
        set_checksum(&mut ack, conn, &[]);
        // ACK 转化成字节流, 调用 tcp_tx 函数发送
        let mut unwritten = &mut buf[..];
        ack.write(&mut unwritten).unwrap();
        let tcp_header_ends_at = buf_len - unwritten.len();
        tcp_tx(conn, &buf[..tcp_header_ends_at]);
        println!("ack: {:?}", &buf[..tcp_header_ends_at]);

        // 更新 tcp 连接状态.
        // todo: 实现累计确认机制
        tcpstate.insert(String::from("ack"), ack.acknowledgment_number as usize);
    }

    // println!("tcp_rx, {:?}, {:?}", conn, std::str::from_utf8(bytes));
}

/// 这个函数会每至少100ms调用一次，以保证控制权可以定期的回到你实现的函数中，而不是一直阻塞在main文件里面。
/// 它可以被用来在不开启多线程的情况下实现超时重传等功能，详见主仓库的README.md
pub fn tick() {
    // TODO 可实现此函数，也可不实现
    // 检查是否超时, 如超时, 重传报文
    // 简化超时机制: 如果相邻两次 tick 调用期间(约 100ms) tcpstate.acked 未改变, 则判断序号为 acked 的包已丢失, 重传序号数在 acked~seq 之间的所有报文
    let mut tcpstate = TCPSTATE.lock().unwrap();
    // let acked = tcpstate.get("acked").unwrap().clone();
    if let Some(&acked) = tcpstate.get("acked"){
        if let Some(&lasttick_acked) = tcpstate.get("lasttick_acked"){
            if acked == lasttick_acked && acked < tcpstate.get("seq").unwrap().clone() {
                println!("acked {:?}超时, seq:{:?}", acked, tcpstate.get("seq").unwrap());
                // 重传 acked 序号报文
                let cache = CACHE.lock().unwrap();
                if let Some(pkt) = cache.get(&(acked as u32)){
                    // let pkt = cache.get(&(acked as u32)).unwrap();
                    // transform vec<u8> into [u8]
                    let mut buf = [0u8; 1500];
                    for i in 0..pkt.len() {
                        buf[i] = pkt[i];
                    }
                    let connection = CONNECTION.lock().unwrap();
                    tcp_tx(&connection, &buf[..pkt.len()]);
                }       
            }
        }
        tcpstate.insert(String::from("lasttick_acked"), acked);
    }
}

// 计算并设置 tcp header 的校验和
pub fn set_checksum(tcp_header: &mut TcpHeader, conn: &ConnectionIdentifier, tcp_payload: &[u8]) {
    let src_ipaddr: Ipv4Addr = conn.src.ip.parse().unwrap();
    let dst_ipaddr: Ipv4Addr = conn.dst.ip.parse().unwrap();

    // 将 tcp header 和 tcp payload 转化为 ipv4 payload 以计算 checksum
    let mut buf = [0u8; 1500];
    let buf_len = buf.len();
    let mut unwritten = &mut buf[..];
    tcp_header.write(&mut unwritten).unwrap();
    let tcp_header_ends_at = buf_len - unwritten.len();
    buf[tcp_header_ends_at..tcp_header_ends_at + tcp_payload.len()].copy_from_slice(tcp_payload);

    // tcp_header.checksum = tcp_header.calc_checksum_ipv4_raw(src_ipaddr.octets(), dst_ipaddr.octets(), &buf[..tcp_header_ends_at + tcp_payload.len()]).expect("failed to compute checksum");
    let tcp_packet = tcp::TcpPacket::new(&buf[..tcp_header_ends_at + tcp_payload.len()]).unwrap();
    tcp_header.checksum = tcp::ipv4_checksum(&tcp_packet, &src_ipaddr, &dst_ipaddr);
    // println!(
    //     "ip payload{:?}",
    //     &buf[..tcp_header_ends_at + tcp_payload.len()]
    // );
}
