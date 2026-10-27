fn main() {
    println!("cargo::rerun-if-changed=src/wrapper.cpp");
    println!("cargo::rerun-if-changed=vendor");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("src/wrapper.cpp")
        .include("vendor")
        .std("c++14")
        .opt_level(2)
        .debug(false)
        .define("NDEBUG", None)
        .warnings(false);
    if std::env::var_os("CARGO_FEATURE_COUNT_ALLOCS").is_some() {
        build.define("FOURFOUR_COUNT_ALLOCS", None);
    }
    build.compile("fourfour_stretch");
}
