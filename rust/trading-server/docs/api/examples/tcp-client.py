"""Minimal QFTX TCP client.

Connects to the trading server on 127.0.0.1:9000, sends one NewOrder
frame, prints the ExecutionReport(s) it gets back.

Usage:
    python3 tcp-client.py
"""

import socket
import struct
import sys
import time

HOST, PORT = "127.0.0.1", 9000
PRICE_SCALE = 100_000_000

HEADER_FMT = "<QQ I I B B 6s"             # seq, ts_ns, magic, length, version, msg_type, _pad
NEW_ORDER_FMT = "<q Q Q Q I B B B B I I"  # price, qty, client_id, order_id, symbol_id,
                                          # side, ord_type, tif, _pad, _pad2, _pad3
EXEC_REPORT_FMT = "<Q Q q Q Q I B B 2s"   # order_id, exec_id, last_price, last_qty,
                                          # leaves_qty, symbol_id, status, side, _pad

HEADER_SIZE      = struct.calcsize(HEADER_FMT)       # 32
NEW_ORDER_SIZE   = struct.calcsize(NEW_ORDER_FMT)    # 48
EXEC_REPORT_SIZE = struct.calcsize(EXEC_REPORT_FMT)  # 48
MAGIC = int.from_bytes(b"QFTX", "little")

STATUS_NAME = {0: "New", 1: "PartiallyFilled", 2: "Filled", 3: "Cancelled", 4: "Rejected"}


def build_new_order(seq, order_id, side, price, qty, symbol_id=0, client_id=1):
    body = struct.pack(
        NEW_ORDER_FMT,
        price * PRICE_SCALE,
        qty,
        client_id,
        order_id,
        symbol_id,
        side,    # 0 = Buy, 1 = Sell
        0,       # ord_type = Limit
        0,       # tif = Day
        0, 0, 0, # padding
    )
    header = struct.pack(
        HEADER_FMT,
        seq,
        time.time_ns(),
        MAGIC,
        NEW_ORDER_SIZE,
        1,                # version
        1,                # msg_type = NewOrder
        b"\x00" * 6,
    )
    return header + body


def recv_exact(sock, n):
    out = b""
    while len(out) < n:
        chunk = sock.recv(n - len(out))
        if not chunk:
            raise EOFError("peer closed during recv")
        out += chunk
    return out


def main():
    with socket.create_connection((HOST, PORT)) as sock:
        sock.sendall(build_new_order(seq=1, order_id=1, side=0, price=100, qty=5))
        # Read one report
        header = recv_exact(sock, HEADER_SIZE)
        seq, ts_ns, magic, length, version, msg_type, _ = struct.unpack(HEADER_FMT, header)
        assert magic == MAGIC, f"bad magic {magic:#x}"
        assert length == EXEC_REPORT_SIZE, f"unexpected body len {length}"
        body = recv_exact(sock, length)
        order_id, exec_id, last_price, last_qty, leaves_qty, symbol_id, status, side, _ = (
            struct.unpack(EXEC_REPORT_FMT, body)
        )
        print(
            f"exec_id={exec_id} order_id={order_id} status={STATUS_NAME.get(status, status)} "
            f"last_qty={last_qty} last_price={last_price / PRICE_SCALE:.2f} "
            f"leaves_qty={leaves_qty}"
        )


if __name__ == "__main__":
    try:
        main()
    except ConnectionRefusedError:
        print(f"trading server not listening on {HOST}:{PORT}", file=sys.stderr)
        sys.exit(1)
