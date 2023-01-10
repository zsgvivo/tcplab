/// 这是等待你完成的代码。正常情况下，本文件是你唯一需要改动的文件。
/// 你可以任意地改动此文件，改动的范围当然不限于已有的五个函数里。（只要已有函数的签名别改，要是签名改了main里面就调用不到了）
/// 在开始写代码之前，请先仔细阅读此文件和api文件。这个文件里的五个函数是等你去完成的，而api里的函数是供你调用的。
/// 提示：TCP是有状态的协议，因此你大概率，会需要一个什么样的数据结构来记录和维护所有连接的状态
use crate::api::{ConnectionIdentifier, app_connected};
// use pnet::packet::tcp;
enum State{
    SYNSENT,
    ESTABLISHED,
    FINWAIT1,
    FINWAIT2,
    TIMEWAIT,
}

pub struct Connection{
    state: State,
    conn: ConnectionIdentifier,
    seq: u32,
    ack: u32,
    // send_window: u16,
    // recv_window: u16,

}

pub struct Tcp {
    pub source: u16,
    pub destination: u16,
    pub sequence: u32,
    pub acknowledgement: u32,
    pub data_offset: u8,
    pub reserved: u8,
    pub flags: u16,
    pub window: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
    // pub options: Vec<TcpOption, Global>,
    // pub payload: Vec<u8, Global>,
}

/// 当有应用想要发起一个新的连接时，会调用此函数。想要连接的对象在conn里提供了。
/// 你应该向想要连接的对象发送SYN报文，执行三次握手的逻辑。
/// 当连接建立好后，你需要调用app_connected函数，通知应用层连接已经被建立好了。
/// param: conn: 连接对象
pub fn app_connect(conn: &ConnectionIdentifier) {
    // TODO 请实现此函数
    tcpsyn = Tcp{
        sourceL: conn.src.
    }
    
    app_connected(conn);
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

    println!("tcp_rx, {:?}, {:?}", conn, std::str::from_utf8(bytes));
}