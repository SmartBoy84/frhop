use std::{process::exit, sync::Arc, thread};

use smol::{
    Executor,
    channel::unbounded,
    future::{self, race},
    lock::RwLock,
};

use crate::{device::TinfoilDevice, listing::Listing};

mod device;
mod game;
mod listing;

const N_THREADS: usize = 4; // turn this up to increase thread count, but come on >4 is overkill for this

fn main() {
    let ex = Arc::new(Executor::new());
    let (signal, shutdown) = unbounded::<()>();
    let shutdown = async move || {
        let _ = shutdown.recv().await;
        print!(".");
    };
    let threads = (0..N_THREADS)
        .map(|_| {
            let ex_clone = ex.clone();
            let shutdown = shutdown.clone();

            thread::spawn(move || future::block_on(ex_clone.run(shutdown())))
        })
        .collect::<Vec<_>>();

    // it's not like tinfoil supports refreshing when its reconnected
    // all this is because on windows device ownership isn't released if we just halt the program
    ctrlc::set_handler(move || {
        println!("shutting down connections");
        // close down executor threads (including main)
        for _ in 0..N_THREADS + 1 {
            let _ = signal.send_blocking(()); // need to do this so the Device object is dropped (release ownership)
        }
    })
    .expect("ctrl-c override failed");

    // main async task
    let ex_clone = ex.clone();
    future::block_on(ex_clone.run(race(shutdown(), async_main(ex))));

    // if we're here, cancel signal sent and tasks finished
    for thread in threads {
        thread.join().unwrap()
    }
    println!("threads closed");
}

async fn async_main(executor: Arc<Executor<'_>>) {
    let mut listing = Listing::new();

    let mut args = std::env::args().skip(1);
    if let Some(d) = args.next() {
        listing.add(d).unwrap();
    } else {
        println!("Specify a [list of] directories or packages to serve");
        exit(-1)
    }

    for d in args {
        listing.add(&d).unwrap();
    }

    if listing.map().is_empty() {
        println!(
            "Either all files specified are invalid packages or all of the directories are empty!"
        );
        exit(-1)
    }

    println!("\n{} nsps found", listing.map().len());

    let listing = Arc::new(RwLock::new(listing));

    println!("Waiting for device connection...");
    loop {
        let tinfoil = loop {
            match TinfoilDevice::wait_new(listing.clone()).await {
                Ok(device) => break device,
                Err(e) => println!("Error connecting: {e:?}"),
            }
        };
        println!("Connected!");
        executor
            .spawn(async {
                if let Err(e) = tinfoil.start_talkin_buddy().await {
                    eprintln!("{e:?} (switch disconnected?)");
                }
            })
            .detach(); // don't need the task
    }

    /*
    if listing is invariant - no need to serialise on every search request
    implement a file watcher?
     */
}
