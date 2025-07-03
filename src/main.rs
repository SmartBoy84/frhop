use std::{process::exit, sync::Arc, thread};

use smol::{
    Executor, Timer,
    channel::{Sender, unbounded},
    future,
    lock::RwLock,
};

use crate::{device::TinfoilDevice, game::listing::Listing};

mod device;
mod game;

const N_THREADS: usize = 10;

fn calm_exit(e: &str) -> ! {
    println!("{e}");
    exit(-1)
}

fn main() {
    let ex = Arc::new(Executor::new());
    let (signal, shutdown) = unbounded::<()>();

    let mut threads = (0..N_THREADS)
        .map(|_| {
            let ex_clone = ex.clone();
            let shutdown = shutdown.clone();

            thread::spawn(move || {
                let _ = future::block_on(ex_clone.run(shutdown.recv()));
            })
        })
        .collect::<Vec<_>>();

    // ctrlc::set_handler(move || {
    //     println!("Ctrl-c!");
    //     let _ = signal.send_blocking(());
    //     for thread in &threads {
    //         thread.join().unwrap()
    //     }
    // })
    // .expect("ctrl-c override failed");

    // main async task
    future::block_on(async_main(ex, signal));
}

async fn async_main(executor: Arc<Executor<'_>>, _signal: Sender<()>) {
    println!("Watching for device connection...");

    let test_dir = "nsp";

    let listing = Listing::from_dir(test_dir).unwrap(); // todo: add a watcher to update
    // todo; add code in TinfoilDevice to reset when file added

    let listing = Arc::new(RwLock::new(listing));

    // let mut tasks = Vec::with_capacity(1);

    loop {
        let tinfoil = loop {
            match TinfoilDevice::wait_new(listing.clone()).await {
                Ok(device) => break device,
                Err(e) => println!("Err: {e:?}"),
            }
        };
        println!("Connected!");
        executor
            .spawn(async {
                if let Err(e) = tinfoil.start_talkin_buddy().await {
                    eprintln!("{e:?}");
                }
            })
            .detach(); // don't need the task
    }

    /*
    if listing is invariant - no need to serialise on every search request
    implement a file watcher?
     */
}
