use clap::Parser;
use env_logger::{Builder, Env, Target};
use futures::{
    channel::mpsc::{channel, Receiver},
    SinkExt, StreamExt,
};
use log::{error, info};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Where to find emulator files
    network_location: String,
}

fn main() {
    let cli = Cli::parse();

    init_logging();
    log_app_name_and_version();

    info!("network location is {}", cli.network_location);

    futures::executor::block_on(async {
        if let Err(e) = async_watch(cli.network_location).await {
            error!("error: {:?}", e)
        }
    });
}

fn init_logging() {
    let env = Env::new()
        .filter_or("MIN_LEVEL", "info")
        .write_style_or("STYLE", "always");
    let mut builder = Builder::from_env(env);
    builder.target(Target::Stdout);
    builder.init();
}

fn log_app_name_and_version() {
    let version = clap::crate_version!();
    let name = clap::crate_name!();
    info!("starting up {name} {version}");
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}

async fn async_watch<P: AsRef<Path>>(path: P) -> notify::Result<()> {
    let (mut watcher, mut rx) = async_watcher()?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    while let Some(res) = rx.next().await {
        match res {
            Ok(event) => info!("changed: {:?}", event),
            Err(e) => error!("watch error: {:?}", e),
        }
    }

    Ok(())
}
