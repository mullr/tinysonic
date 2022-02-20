use cstr::cstr;
use qmetaobject::{prelude::*, qml_register_singleton_instance};
use serde::Deserialize;
use tokio::sync::mpsc::unbounded_channel;

mod albums;
mod audio;
mod comms;
mod output;
mod player;
mod plm;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub url: String,
    pub username: String,
    pub password: String,
}

qrc!(my_resource,
    "tinysonic/qml" {
        "ui/main.qml",
        "ui/shadow.png",
        "ui/AlbumCover.qml",
        "ui/AlbumCoverGridItem.qml",
        "ui/PlayingBar.qml",
    },
);

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

    let (comms_tx, comms_rx) = unbounded_channel::<comms::Request>();
    let (plm_tx, plm_rx) = unbounded_channel::<plm::PlmCommand>();

    tokio::spawn(async move { comms::run(config, comms_rx).await });

    let comms_tx2 = comms_tx.clone();
    let plm_tx2 = plm_tx.clone();
    tokio::spawn(async move { plm::PlmTask::new(plm_tx2, plm_rx, comms_tx2).run().await });

    let _ = tokio::task::spawn_blocking(move || {
        qmetaobject::log::init_qt_to_rust();

        let albums = albums::Albums::new(comms_tx.clone());
        let player = player::Player::new(comms_tx, plm_tx);

        my_resource();
        qml_register_singleton_instance(
            cstr!("io.github.mullr.tinysonic"),
            1,
            0,
            cstr!("Albums"),
            albums,
        );

        qml_register_singleton_instance(
            cstr!("io.github.mullr.tinysonic"),
            1,
            0,
            cstr!("Player"),
            player,
        );

        let mut engine = QmlEngine::new();

        engine.load_file("qrc:/tinysonic/qml/ui/main.qml".into());

        engine.exec();
    })
    .await;
}
