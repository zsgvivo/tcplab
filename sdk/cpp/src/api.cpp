/**
 * 这是SDK内置的代码，本文件是api.h中函数的实现。你只读api.h就够了。
 * 此文件通常不用改动。但如果你有确切的理由，也可以自行改动，但请务必确保你清楚自己在做什么！
 * 助教评阅时，会使用你上传的版本。
 */
#include "api.h"

void sdk_event(ConnectionIdentifier &conn, std::vector<uint8_t> &bytes, uint32_t flags);

void app_connected(ConnectionIdentifier &conn) {
    std::vector<uint8_t> _;
    sdk_event(conn, _, 0x2);
}

void release_connection(ConnectionIdentifier &conn) {
    std::vector<uint8_t> _;
    sdk_event(conn, _, 0x80);
}

void app_recv(ConnectionIdentifier &conn, std::vector<uint8_t> &bytes) {
    sdk_event(conn, bytes, 0x0);
}

void app_peer_fin(ConnectionIdentifier &conn) {
    std::vector<uint8_t> _;
    sdk_event(conn, _, 0x1);
}

void app_peer_rst(ConnectionIdentifier &conn) {
    std::vector<uint8_t> _;
    sdk_event(conn, _, 0x4);
}

void tcp_tx(ConnectionIdentifier &conn, std::vector<uint8_t> &bytes) {
    sdk_event(conn, bytes, 0x40);
}

std::ostream &operator<<(std::ostream &out, ConnectionIdentifier &conn) {
    out << "(" << conn.src.ip << ":" << conn.src.port << " -> " << conn.dst.ip << ":" << conn.dst.port << ")";
    return out;
}
