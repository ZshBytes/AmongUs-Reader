fn main() {
    println!("cargo:rerun-if-changed=offsets.toml");
    println!("cargo:rerun-if-changed=offsets.example.toml");
}
