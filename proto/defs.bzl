def rust_protobuf_library(name, srcs, build_script, protos, **kwargs):
    build_name = name + "-build"
    proto_name = name + "-proto"

    native.rust_binary(
        name = build_name,
        srcs = [build_script],
        crate_root = build_script,
        deps = [
            "//third-party/rust:tonic-build",
        ],
    )

    native.genrule(
        name = proto_name,
        srcs = protos,
        cmd = "$(exe :{})".format(build_name),
        env = {
            "PROTOC": "$(exe //third-party/protobuf:protoc)",
            "PROTOC_INCLUDE": "$(location //third-party/protobuf:google_protobuf)",
        },
        out = ".",
    )

    native.rust_library(
        name = name,
        srcs = srcs,
        doctests = False,
        env = {
            "OUT_DIR": "$(location :{})".format(proto_name),
        },
        **kwargs
    )
