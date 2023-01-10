/// 这是等待你完成的代码。正常情况下，本文件是你唯一需要改动的文件。
/// 你可以任意地改动此文件，改动的范围当然不限于已有的五个函数里。（只要已有函数的签名别改，要是签名改了main里面就调用不到了）
/// 在开始写代码之前，请先仔细阅读此文件和api文件。这个文件里的五个函数是等你去完成的，而api里的函数是供你调用的。
/// 提示：TCP是有状态的协议，因此你大概率，会需要一个什么样的数据结构来记录和维护所有连接的状态
use crate::api::{ConnectionIdentifier, app_connected, tcp_tx};
use pnet::packet::tcp;
use etherparse::{TcpHeader, TcpHeaderSlice};
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::collections::HashMap;

// enum State{
//     CLOSED,
//     SYNSENT,
//     ESTABLISHED,
//     FINWAIT1,
//     FINWAIT2,
//     TIMEWAIT,
// }

// pub struct Connection{
//     state: State,
//     conn: ConnectionIdentifier,
//     seq: u32,
//     ack: u32,
//     // send_window: u16,
//     // recv_window: u16,
// }


const CLOSED:usize = 0;
const SYNSENT:usize = 1;
const ESTABLISHED:usize = 2;
const FINWAIT1:usize = 3;
const FINWAIT2:usize = 4;
const TIMEWAIT:usize = 5;
lazy_static!{
    // 存储 TCP 连接状态的全局变量
    static ref TCPSTATE:Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());
}

/// 当有应用想要发起一个新的连接时，会调用此函数。想要连接的对象在conn里提供了。
/// 你应该向想要连接的对象发送SYN报文，执行三次握手的逻辑。
/// 当连接建立好后，你需要调用app_connected函数，通知应用层连接已经被建立好了。
/// param: conn: 连接对象
pub fn app_connect(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    let mut buf = [0u8; 1500];
    // 构造 TCP SYN 报文
    let mut syn = TcpHeader::new(
        conn.src.port,
        conn.dst.port,
        0,
        0,
    );
    syn.syn = true;

    // 转化并发送报文字节流
    let mut tcp_header_buf = &mut buf[..syn.header_len() as usize];
    syn.write(&mut tcp_header_buf).unwrap();
    tcp_tx(conn, &buf[..syn.header_len() as usize]);
    println!("tcp_header_buf: {:?}", &buf[..syn.header_len() as usize]);

    // 更新 TCP 连接状态
    let mut tcpstate = TCPSTATE.lock().unwrap();
    tcpstate.insert(String::from("state"), SYNSENT); // tcp 连接当前在 FSM 图中的位置
    tcpstate.insert(String::from("seq"), 0); // 序号数
    tcpstate.insert(String::from("ack"), 0); // ACK 数
    
    println!("app_connect, {:?}", conn);
}

/// 当应用层想要在一个已经建立好的连接上发送数据时，会调用此函数。
/// param: conn: 连接对象
///        bytes: 数据内容，是字节数组
pub fn app_send(conn: &ConnectionIdentifier, bytes: &[u8]) {
    // TODO 请实现此函数
    println!("app_send, {:?}, {:?}", conn, std::str::from_utf8(bytes));
}

/// 当应用层想要半关闭连接(FIN)时，会调用此函数。
/// param: conn: 连接对象
pub fn app_fin(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    println!("app_fin, {:?}", conn);
}

/// 当应用层想要重置连接(RES)时，会调用此函数
/// param: conn: 连接对象
pub fn app_rst(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    println!("app_rst, {:?}", conn);
}

/// 当收到TCP报文时，会调用此函数。
/// 正常情况下，你会对TCP报文，根据报文内容和连接的当前状态加以处理，然后调用0个~多个api文件中的函数
/// param: conn: 连接对象
///        bytes: TCP报文内容，是字节数组。（含TCP报头，不含IP报头）
pub fn tcp_rx(conn: &ConnectionIdentifier, bytes: &[u8]) {
    // TODO 请实现此函数
    // 处理收到的 tcp 报文
    let tcp_header = TcpHeader::from_slice(bytes).unwrap().0;
    println!("rx tcp_header: {:?}", tcp_header);
    // 如果收到 SYNACK 报文
    if tcp_header.syn == true && tcp_header.ack == true{
        // 构造 ACK 报文头
        let mut tcpstate = TCPSTATE.lock().unwrap();
        let mut buf = [0u8; 1500];
        let mut ack = TcpHeader::new(
            conn.src.port,
            conn.dst.port,
            tcp_header.acknowledgment_number,
            1000,
        );
        ack.ack = true;
        ack.acknowledgment_number = tcp_header.sequence_number + 1;

        // 转化成字节流, 调用 tcp_tx 函数发送
        let mut tcp_header_buf = &mut buf[..ack.header_len() as usize];
        ack.write(&mut tcp_header_buf).unwrap();
        tcp_tx(conn, &buf[..ack.header_len() as usize]);

        // 更新 tcp 连接状态
        tcpstate.insert(String::from("state"), ESTABLISHED);
        tcpstate.insert(String::from("ack"), ack.acknowledgment_number as usize);
        tcpstate.insert(String::from("seq"), ack.sequence_number as usize);

        // 调用app_connected函数，通知应用层连接已经被建立好了
        app_connected(conn);
    }
    println!("tcp_rx, {:?}, {:?}", conn, std::str::from_utf8(bytes));
}