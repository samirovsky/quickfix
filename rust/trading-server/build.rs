fn main() {
    println!("cargo:rerun-if-changed=proto/trading.proto");
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(&["proto/trading.proto"], &["proto"])
        .expect("tonic-build: compile trading.proto");
}
