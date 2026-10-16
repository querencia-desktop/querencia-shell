// HMP-09: the desktop shares the panel's classic_menu.rs; this cfg leaves
// out the panel's own GUI test module there, which needs the panel's modules.
fn main() {
    println!("cargo::rustc-cfg=querencia_desktop");
}
