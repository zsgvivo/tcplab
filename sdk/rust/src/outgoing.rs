/// 这是等待你完成的代码。正常情况下，本文件是你唯一需要改动的文件。
/// 你可以任意地改动此文件，改动的范围当然不限于已有的五个函数里。（只要已有函数的签名别改，要是签名改了main里面就调用不到了）
/// 在开始写代码之前，请先仔细阅读此文件和api文件。这个文件里的五个函数是等你去完成的，而api里的函数是供你调用的。
/// 提示：TCP是有状态的协议，因此你大概率，会需要一个什么样的数据结构来记录和维护所有连接的状态
use crate::api::{
    app_connected, app_peer_fin, app_peer_rst, app_recv, release_connection, tcp_tx,
    ConnectionIdentifier,
};
use etherparse::TcpHeader;
use lazy_static::lazy_static;
use pnet::packet::tcp;
use std::cmp::min;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::time;

#[derive(Clone, Copy)]
enum FSMState {
    // CLOSED,
    SYNSENT,
    ESTABLISHED,
    FINWAIT1,
    FINWAIT2,
    TIMEWAIT,
}

struct TCP_packet {
    length: u32,
    buf: [u8; 1500],
}

struct TCPState {
    // 存储一条 TCP 连接状态的结构
    // state: tcp 连接当前在 FSM 图中的位置
    // seq: 下一个发送的 tcp 报文序号数
    // ack: 发送的 ack 序号数
    // acked: 未被确认的最小序号数
    // dup_acks: 重复 ack 个数,超过三次快速重传.
    // send_time: 每条报文的发送时间
    // cache: 缓存发送过的报文以备超时重传
    state: FSMState,
    seq: u32,
    ack: u32,
    acked: u32,
    dup_acks: usize,
    send_times: HashMap<u32, time::Instant>,
    cache: HashMap<u32, TCP_packet>,
}

const MSS: usize = 1460; // TCP 报文最大长度, 取典型值1460
const WINDOW: u16 = MSS as u16; // 暂未实现 tcp 缓存机制

lazy_static! {
    // 存储所有 TCP 连接的状态
    static ref TCPSTATES:Mutex<HashMap<String, TCPState>> = Mutex::new(HashMap::new());
    // 存储所有在线的 TCP 连接
    static ref CONNECTIONS:Mutex<Vec<ConnectionIdentifier>> = Mutex::new(Vec::new());
}

/// 当有应用想要发起一个新的连接时，会调用此函数。想要连接的对象在conn里提供了。
/// 你应该向想要连接的对象发送SYN报文，执行三次握手的逻辑。
/// 当连接建立好后，你需要调用app_connected函数，通知应用层连接已经被建立好了。
/// param: conn: 连接对象
pub fn app_connect(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    let mut buf = [0u8; 1500];
    // 构造 TCP SYN 报文
    let mut syn = TcpHeader::new(conn.src.port, conn.dst.port, 0, WINDOW);
    syn.syn = true;
    set_checksum(&mut syn, conn, &[]);

    // 转化并发送报文字节流
    let mut tcp_header_buf = &mut buf[..syn.header_len() as usize];
    syn.write(&mut tcp_header_buf).unwrap();
    tcp_tx(conn, &buf[..syn.header_len() as usize]);

    // 初始化更新 TCP 连接状态
    let tcp_state = TCPState {
        state: FSMState::SYNSENT,
        seq: 1,
        ack: 0,
        acked: 0,
        dup_acks: 0,
        send_times: HashMap::new(),
        cache: HashMap::new(),
    };
    let mut tcpstates = TCPSTATES.lock().unwrap();
    tcpstates.insert(ConnectionIdentifier2Str(&conn), tcp_state);
    // 注册新 tcp 连接
    let mut connections = CONNECTIONS.lock().unwrap();
    connections.push(conn.clone())

    // println!("app_connect, {:?}", conn);
}

/// 当应用层想要在一个已经建立好的连接上发送数据时，会调用此函数。
/// param: conn: 连接对象
///        bytes: 数据内容，是字节数组
pub fn app_send(conn: &ConnectionIdentifier, bytes: &[u8]) {
    // TODO 请实现此函数
    // 这里应用层数据 bytes 的长度可能超过 MSS, 需要进行分片, 否则会 panic
    let n = bytes.len() / MSS + 1;
    for i in 0..n {
        let payload = &bytes[i * MSS..min((i + 1) * MSS, bytes.len())];
        let mut tcpstates = TCPSTATES.lock().unwrap();
        let tcp_state = tcpstates.get_mut(&ConnectionIdentifier2Str(&conn)).unwrap();

        // 构造带 payload 的tcp 数据报
        let mut tcp_header = TcpHeader::new(conn.src.port, conn.dst.port, tcp_state.seq, WINDOW);
        // tcp_header.ack = true;
        tcp_header.acknowledgment_number = tcp_state.ack;
        set_checksum(&mut tcp_header, conn, payload);

        let mut buf = [0u8; 1500];
        let buf_len = buf.len();
        let mut unwritten = &mut buf[..];
        tcp_header.write(&mut unwritten).unwrap();
        let tcp_header_ends_at = buf_len - unwritten.len();
        buf[tcp_header_ends_at..tcp_header_ends_at + payload.len()].copy_from_slice(payload);
        tcp_tx(conn, &buf[..tcp_header_ends_at + payload.len()]);

        // 更新 tcp 连接状态
        tcp_state.seq += payload.len() as u32;
        tcp_state
            .send_times
            .insert(tcp_header.sequence_number, time::Instant::now());
        tcp_state.cache.insert(
            tcp_header.sequence_number,
            TCP_packet {
                length: (tcp_header_ends_at + payload.len()) as u32,
                buf: buf,
            },
        );
    }

    // println!("app_send, {:?}, {:?}", conn, std::str::from_utf8(bytes));
}

/// 当应用层想要半关闭连接(FIN)时，会调用此函数。
/// param: conn: 连接对象
pub fn app_fin(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    // 构造 TCP FIN 报文
    let mut tcpstates = TCPSTATES.lock().unwrap();
    let tcp_state = tcpstates.get_mut(&ConnectionIdentifier2Str(&conn)).unwrap();
    let mut fin = TcpHeader::new(conn.src.port, conn.dst.port, tcp_state.seq, WINDOW);
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
    tcp_state.state = FSMState::FINWAIT1;

    // println!("app_fin, {:?}", conn);
}

/// 当应用层想要重置连接(RES)时，会调用此函数
/// param: conn: 连接对象
pub fn app_rst(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    // 构造 TCP RST 报文
    let mut tcpstates = TCPSTATES.lock().unwrap();
    let tcp_state = tcpstates.get_mut(&ConnectionIdentifier2Str(&conn)).unwrap();
    let mut rst = TcpHeader::new(conn.src.port, conn.dst.port, tcp_state.seq, WINDOW);
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
    let mut tcpstates = TCPSTATES.lock().unwrap();
    let tcp_state = tcpstates.get_mut(&ConnectionIdentifier2Str(&conn)).unwrap();
    // 如果收到 SYNACK 报文
    if tcp_header.syn == true && tcp_header.ack == true {
        // 构造 ACK 报文头
        let mut buf = [0u8; 1500];
        let mut ack = TcpHeader::new(
            conn.src.port,
            conn.dst.port,
            tcp_header.acknowledgment_number,
            WINDOW,
        );
        ack.ack = true;
        ack.acknowledgment_number = tcp_header.sequence_number + 1;
        set_checksum(&mut ack, conn, &[]);

        // 转化成字节流, 调用 tcp_tx 函数发送
        let mut tcp_header_buf = &mut buf[..ack.header_len() as usize];
        ack.write(&mut tcp_header_buf).unwrap();
        tcp_tx(conn, &buf[..ack.header_len() as usize]);

        // 更新 tcp 连接状态
        tcp_state.state = FSMState::ESTABLISHED;
        tcp_state.seq = ack.sequence_number;
        tcp_state.ack = ack.acknowledgment_number;
        tcp_state.acked = tcp_header.acknowledgment_number;

        // 调用app_connected函数，通知应用层连接已经被建立好了
        app_connected(conn);
    }
    // 如果收到普通 ACK
    else if tcp_header.ack == true {
        // 判断此 ack 是否重复
        if tcp_header.acknowledgment_number == tcp_state.acked{
            tcp_state.dup_acks += 1;
        }
        else{
            tcp_state.dup_acks = 0;
        }
        

        // 更新 tcp 连接状态
        tcp_state.acked = tcp_header.acknowledgment_number;

        // 若收到的 ack 已经重复 3 次,重传报文.
        if tcp_state.dup_acks == 3{
            if let Some(pkt) = tcp_state.cache.get(&tcp_state.acked){
                tcp_tx(conn, &pkt.buf[..pkt.length as usize]);
            }
        }

        // 如果是 FIN ACK, 更新状态
        if let FSMState::FINWAIT1 = tcp_state.state {
            tcp_state.state = FSMState::FINWAIT2;
        }
    }
    // 如果收到对方发来的 FIN 报文
    if tcp_header.fin == true {
        // 回复 ACK 报文
        if tcp_header.sequence_number == tcp_state.ack {
            // 调用 app_peer_fin 函数，通知应用层对端半关闭连接
            app_peer_fin(conn);
            let mut buf = [0u8; 1500];
            let mut ack = TcpHeader::new(
                conn.src.port,
                conn.dst.port,
                tcp_header.acknowledgment_number,
                WINDOW,
            );
            ack.ack = true;
            ack.acknowledgment_number = tcp_header.sequence_number + 1;
            set_checksum(&mut ack, conn, &[]);

            // 转化成字节流, 调用 tcp_tx 函数发送
            let mut tcp_header_buf = &mut buf[..ack.header_len() as usize];
            ack.write(&mut tcp_header_buf).unwrap();
            tcp_tx(conn, &buf[..ack.header_len() as usize]);

            // 更新 tcp 连接状态并通知应用层
            if let FSMState::FINWAIT2 = tcp_state.state {
                tcp_state.state = FSMState::TIMEWAIT;
                // 完成四次挥手, 通知 driver 释放连接
                release_connection(conn);
                // 从 CONNECTIONS 中删除该 tcp 条目
                // let mut connections = CONNECTIONS.lock().unwrap();
                //
            }
        }
    }
    // 如果收到对方发来的 RST 报文
    if tcp_header.rst == true {
        // 通知应用层
        app_peer_rst(conn);
    }
    // 如果有 payload 数据
    if payload.len() > 0 {
        // 如果收到想要的数据, 更新确认序号ack
        if tcp_header.sequence_number == tcp_state.ack
        // 调用 app_recv 函数，通知应用层收到了数据
        {
            app_recv(conn, payload);
            // 更新 tcp 连接状态
            tcp_state.ack = tcp_header.sequence_number + payload.len() as u32;
        }
        // 回复 ACK 报文
        let mut buf = [0u8; 1500];
        let buf_len = buf.len();
        let mut ack = TcpHeader::new(conn.src.port, conn.dst.port, tcp_state.seq, WINDOW);
        ack.ack = true;
        ack.acknowledgment_number = tcp_state.ack;
        set_checksum(&mut ack, conn, &[]);
        // ACK 转化成字节流, 调用 tcp_tx 函数发送
        let mut unwritten = &mut buf[..];
        ack.write(&mut unwritten).unwrap();
        let tcp_header_ends_at = buf_len - unwritten.len();
        tcp_tx(conn, &buf[..tcp_header_ends_at]);
    }
    // println!("tcp_rx, {:?}, {:?}", conn, std::str::from_utf8(bytes));
}

/// 这个函数会每至少100ms调用一次，以保证控制权可以定期的回到你实现的函数中，而不是一直阻塞在main文件里面。
/// 它可以被用来在不开启多线程的情况下实现超时重传等功能，详见主仓库的README.md
pub fn tick() {
    // TODO 可实现此函数，也可不实现
    // 检查每个 tcp 连接的 acked 序号报文段是否超时, 如超时, 重传相应报文
    // 暂时设定超时上限 1s, todo: 上限1.5 estRTT or 1estRTT + 4DevRTT
    let mut tcpstates = TCPSTATES.lock().unwrap();
    let connections = CONNECTIONS.lock().unwrap();
    for i in 0..connections.len() {
        let conn = &connections[i];
        let tcp_state = tcpstates.get_mut(&ConnectionIdentifier2Str(&conn)).unwrap();
        if let Some(&acked_send_time) = tcp_state.send_times.get(&tcp_state.acked) {
            if let FSMState::ESTABLISHED = tcp_state.state {
                let time_now = time::Instant::now();
                if time_now - time::Duration::from_millis(300) > acked_send_time && tcp_state.acked < tcp_state.seq{
                    println!("超时重传!{:?}", time::Instant::now());
                    // acked 序号报文超时, 重传报文
                    let pkt = tcp_state.cache.get(&tcp_state.acked).unwrap();
                    tcp_tx(conn, &pkt.buf[..pkt.length as usize]);
                    // 更新该报文的发送时间
                    tcp_state
                    .send_times
                    .insert(tcp_state.acked, time::Instant::now());
                }
            }
        }
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
    let tcp_packet = tcp::TcpPacket::new(&buf[..tcp_header_ends_at + tcp_payload.len()]).unwrap();
    // 设置 checksum
    tcp_header.checksum = tcp::ipv4_checksum(&tcp_packet, &src_ipaddr, &dst_ipaddr);
}

pub fn ConnectionIdentifier2Str(conn: &ConnectionIdentifier) -> String {
    // 把 ConnectionIdentifier 转换成 String 以便哈希, 格式: 0.0.0.0:0->1.1.1.1:1
    let src_ipaddr: Ipv4Addr = conn.src.ip.parse().unwrap();
    let dst_ipaddr: Ipv4Addr = conn.dst.ip.parse().unwrap();
    let src_port = conn.src.port;
    let dst_port = conn.dst.port;
    let src_ip = src_ipaddr.to_string();
    let dst_ip = dst_ipaddr.to_string();
    let src = format!("{}:{}", src_ip, src_port);
    let dst = format!("{}:{}", dst_ip, dst_port);
    let conn_str = format!("{}->{}", src, dst);
    conn_str
}