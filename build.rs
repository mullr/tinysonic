use rust_qt_binding_generator::build::QtModule;

fn main() {
    let out_dir = ::std::env::var("OUT_DIR").unwrap();

    rust_qt_binding_generator::build::Build::new(&out_dir)
        .bindings("bindings.json")
        .qrc("qml.qrc")
        .cpp("src/main.cpp")
        .module(QtModule::Gui)
        .module(QtModule::Qml)
        .module(QtModule::QuickControls2)
        .compile("tinysonic");

    // TODO rust_qt_binding_generator::build should do this automatically
    println!("cargo:rerun-if-changed=bindings.json");
    println!("cargo:rerun-if-changed=qml.qrc");
    println!("cargo:rerun-if-changed=src/main.cpp");
}
