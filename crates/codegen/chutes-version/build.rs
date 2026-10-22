fn main() {
    println!("cargo:rerun-if-env-changed=CHUTES_BUILD_VERSION");
}
