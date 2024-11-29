// pub mod solana_transaction_decode;
// mod program_pumpfun;

// fn main() {
//     let out_dir = "./proto"; // Specify your custom output directory
//     //prost_build::compile_protos(&["proto/event.proto"], &["proto"]).unwrap();
//     prost_build::compile_protos(&["proto/event.proto"], &[out_dir]).unwrap();
// }

use prost_build::compile_protos;
use tonic_build::configure;

fn main() {
    const PROTOC_ENVAR: &str = "PROTOC";
    if std::env::var(PROTOC_ENVAR).is_err() {
        #[cfg(not(windows))]
        std::env::set_var(PROTOC_ENVAR, protobuf_src::protoc());
    }

    configure()
        .compile_protos(
            &[
                "proto/auth.proto",
                "proto/shared.proto",
                "proto/shredstream.proto",
                "proto/trace_shred.proto",
            ],
            &["protos"],
        )
        .unwrap();
}
