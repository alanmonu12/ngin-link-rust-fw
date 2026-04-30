fn main() {
    // Le pasamos las banderas necesarias al linker (enlazador) para sistemas embebidos
    println!("cargo:rustc-link-arg-bins=--nmagic");
}