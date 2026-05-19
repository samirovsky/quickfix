// Compiles the gRPC client stubs against the QFTX TradingService proto.
//
// The proto file is **copied** from `rust/trading-server/proto/trading.proto`
// rather than referenced because Cargo doesn't have first-class cross-crate
// proto sharing today. Keep the two files identical; this comment is the
// best we can do until we move both crates into a workspace.

fn main() {
    println!("cargo:rerun-if-changed=proto/trading.proto");
    tonic_build::configure()
        // We're a client only — no server impls needed in this crate.
        .build_server(false)
        .build_client(true)
        .compile_protos(&["proto/trading.proto"], &["proto"])
        .expect("tonic-build: compile trading.proto");
}
