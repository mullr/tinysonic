use std::sync::Arc;

use library::Library;
use serde::Deserialize;
use tokio::sync::mpsc::unbounded_channel;

mod audio;
mod library;
mod output;
mod plm;

pub mod ui_interface {
    include!(concat!(env!("OUT_DIR"), "/src/ui_interface.rs"));
}
mod ui_impl;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub url: String,
    pub username: String,
    pub password: String,
}

extern "C" {
    fn main_cpp(app: *const ::std::os::raw::c_char, library: u64, plm_tx: u64);
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let config_path = dirs::config_dir()
        .expect("Can't resolve system config dir")
        .join("tinysonic")
        .join("config.toml");

    let config: Config = toml::from_str(
        &std::fs::read_to_string(config_path).expect("Can't read config file at {config_path}"),
    )
    .expect("Bad config file format");

    let library = Arc::new(Library::new(config));

    let (plm_tx, plm_rx) = unbounded_channel::<plm::PlmCommand>();
    let plm_tx2 = plm_tx.clone();
    let lib = library.clone();
    tokio::spawn(async move { plm::PlmTask::new(plm_tx2, plm_rx, lib).run().await });

    tokio::task::spawn_blocking(move || {
        use std::ffi::CString;
        let app_name = ::std::env::args().next().unwrap();
        let app_name = CString::new(app_name).unwrap();
        unsafe {
            main_cpp(
                app_name.as_ptr(),
                &library as *const _ as u64,
                &plm_tx as *const _ as u64,
            );
        }
    })
    .await
    .unwrap();
}
