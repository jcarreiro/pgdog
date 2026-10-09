use crate::backend::pool::Request;

use super::Connection;

#[tokio::test(flavor = "multi_thread")]
async fn parameters_survive_back_to_back_reloads() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::backend::databases::reload_from_existing;

    crate::config::load_test();

    // Each reload takes the previous pools offline, so a client that checks
    // out while reloads land back to back sees its pools replaced more than
    // once.
    let reloading = Arc::new(AtomicBool::new(true));
    let reloads = {
        let reloading = reloading.clone();
        tokio::task::spawn_blocking(move || {
            for _ in 0..50 {
                reload_from_existing().unwrap();
            }
            reloading.store(false, Ordering::Relaxed);
        })
    };

    let clients: Vec<_> = (0..16)
        .map(|_| {
            let reloading = reloading.clone();
            tokio::spawn(async move {
                let mut failures = 0;
                while reloading.load(Ordering::Relaxed) {
                    let mut conn = Connection::new("pgdog", "pgdog", false).unwrap();
                    if conn.parameters(&Request::default()).await.is_err() {
                        failures += 1;
                    }
                }
                failures
            })
        })
        .collect();

    reloads.await.unwrap();
    let mut failures = 0;
    for client in clients {
        failures += client.await.unwrap();
    }
    assert_eq!(failures, 0, "clients failed while pools were reloading");
}
