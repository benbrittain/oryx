fn get_env(key: &str) -> Option<std::ffi::OsString> {
    println!("cargo:rerun-if-env-changed={}", key);
    std::env::var_os(key)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tonic = tonic_build::configure()
        .protoc_arg("--experimental_allow_proto3_optional")
        .build_server(true);

    if get_env("OUT_DIR").is_none() {
        if let Some(out) = get_env("OUT") {
            tonic = tonic.out_dir(out);
        }
    }

    let args: Vec<_> = std::env::args().collect();
    if args.len() > 1 {
        println!("The first argument is {}", args[1]);
    }

    tonic.compile(
        &[
            // TODO take from env var or something
            "build/bazel/remote/execution/v2/remote_execution.proto",
            "build/bazel/semver/semver.proto",
            "google/api/annotations.proto",
            "google/api/client.proto",
            "google/api/http.proto",
            "google/bytestream/bytestream.proto",
            "google/longrunning/operations.proto",
            "google/rpc/code.proto",
            "google/rpc/status.proto",
            "google/rpc/error_details.proto",
        ],
        &["."],
    )?;
    Ok(())
}
