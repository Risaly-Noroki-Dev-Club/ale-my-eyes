fn main() {
    slint_build::compile("ui/app.slint").unwrap();

    // 当 Java 源码或 Android 资源变化时重新编译
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" {
        println!("cargo:rerun-if-changed=android/java/");
        println!("cargo:rerun-if-changed=android/res/");
    }
}
