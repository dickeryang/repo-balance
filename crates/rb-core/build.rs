fn main() {
    // libgit2 vendored 构建在 Windows 上依赖 advapi32（SID/Token/Crypt/Reg），
    // libgit2-sys 在 default-features=false 下未输出该链接指令，此处补齐。
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
}