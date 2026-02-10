fn main() {
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-search=framework=/Library/Frameworks");
    println!("cargo:rustc-link-lib=framework=3DconnexionClient");
}
