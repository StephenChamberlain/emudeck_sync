use clap::Parser;
use env_logger::{Builder, Env, Target};
use fs_extra::dir::{self, CopyOptions, TransitProcess};
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
    local_emulation_directory: String,

    /// Where to find emulator files
    network_emulation_directory: String,
}

fn main() {
    let cli = Cli::parse();

    init_logging();
    log_app_name_and_version();
    log_emulation_locations(&cli);

    // The EmuDeck installation might have been updated, make sure the network file system is
    // up to date. New ROMs can go in the appropriate directories. This also ensures saves are
    // pushed to the NAS.
    sync_emudeck_to_network_directories(&cli);

    futures::executor::block_on(async {
        if let Err(e) = async_watch(cli.network_emulation_directory).await {
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

fn log_emulation_locations(cli: &Cli) {
    info!(
        "local emulation directory: {}",
        cli.local_emulation_directory
    );
    info!(
        "network emulation directory: {}",
        cli.network_emulation_directory
    );
}

fn sync_emudeck_to_network_directories(cli: &Cli) {
    let mut options = CopyOptions::new();
    options.overwrite = false;
    options.skip_exist = true;
    options.copy_inside = false;
    options.content_only = true;

    info!("syncing network emulation directory with the local emulation directory structure");

    match sync_directories(
        options,
        cli.local_emulation_directory.as_str(),
        &cli.network_emulation_directory.as_str(),
    ) {
        Ok(_event) => {}
        Err(e) => error!("directory sync error: {:?}", e),
    }
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

    info!("starting network emulation directory watcher...");

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    while let Some(res) = rx.next().await {
        match res {
            Ok(event) => handle_file_system_event(event),
            Err(e) => error!("watch error: {:?}", e),
        }
    }

    Ok(())
}

fn handle_file_system_event(event: Event) {
    info!("event: {:?}", event)
}

fn sync_directories(options: CopyOptions, source: &str, destination: &str) -> std::io::Result<()> {
    let destination_path = Path::new(destination);
    if !destination_path.exists() {
        info!(
            "network emulation directory: {} does not exist, creating",
            destination_path.display()
        );
        std::fs::create_dir_all(destination)?;
    }

    let handle = |process_info: TransitProcess| {
        let percentage =
            (process_info.copied_bytes as f64 / process_info.total_bytes as f64) * 100.0;

        // Log progress around every 10 percent
        if (percentage as u32) % 10 == 0 {
            info!(
                "emulation folder synchronisation progress: {:.2}%",
                percentage
            );
        }
        fs_extra::dir::TransitProcessResult::ContinueOrAbort
    };

    // Sync the directories by copying from source to destination
    match dir::copy_with_progress(Path::new(source), Path::new(destination), &options, handle) {
        Ok(_result) => {}
        Err(e) => error!("sync directories error: {:?}", e),
    }

    Ok(())
}
