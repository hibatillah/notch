include!("../build_support/icon_resource.rs");

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=../build_support/icon_resource.rs");
    compile_icon_resource("../assets/branding", "settings.rc")
}
