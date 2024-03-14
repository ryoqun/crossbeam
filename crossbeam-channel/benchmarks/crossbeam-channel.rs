use crossbeam_channel::{bounded, unbounded, Receiver, Select, Sender};

mod message;

const MESSAGES: usize = 500_000;
const THREADS: usize = 4;

fn new<T>(cap: Option<usize>) -> (Sender<T>, Receiver<T>) {
    match cap {
        None => unbounded(),
        Some(cap) => bounded(cap),
    }
}

fn seq(cap: Option<usize>) {
    let (tx, rx) = new(cap);

    for i in 0..MESSAGES {
        tx.send(message::new(i)).unwrap();
    }

    for _ in 0..MESSAGES {
        rx.recv().unwrap();
    }
}

fn spsc(cap: Option<usize>) {
    let (tx, rx) = new(cap);

    crossbeam::scope(|scope| {
            let core_id = core_id();
            let tx = tx.clone();
        scope.spawn(move |_| {
            set_for_current(core_id);
            for i in 0..MESSAGES {
                tx.send(message::new(i)).unwrap();
            }
        });

        for _ in 0..MESSAGES {
            rx.recv().unwrap();
        }
    })
    .unwrap();
}

fn mpsc(cap: Option<usize>) {
    let (tx, rx) = new(cap);

    crossbeam::scope(|scope| {
        for _ in 0..THREADS {
            let core_id = core_id();
            let tx = tx.clone();
            scope.spawn(move |_| {
            set_for_current(core_id);
                for i in 0..MESSAGES / THREADS {
                    tx.send(message::new(i)).unwrap();
                }
            });
        }

        for _ in 0..MESSAGES {
            rx.recv().unwrap();
        }
    })
    .unwrap();
}

fn spmc(cap: Option<usize>) {
    let (tx, rx) = new(cap);

    crossbeam::scope(|scope| {
        for i in 0..MESSAGES {
            tx.send(message::new(i)).unwrap();
        }

        for _ in 0..THREADS {
            let core_id = core_id();
            let rx = rx.clone();
            scope.spawn(move |_| {
            set_for_current(core_id);
                for _ in 0..MESSAGES / THREADS {
                    rx.recv().unwrap();
                }
            });
        }
    })
    .unwrap();
}

use std::sync::Mutex;
use core_affinity::{CoreId, set_for_current};

fn core_id() -> CoreId {
    static CPU_IDS: std::sync::Mutex<Vec<CoreId>> = Mutex::new(Vec::new());
    let mut v = CPU_IDS.lock().unwrap();
    if v.is_empty() {
        *v = core_affinity::get_core_ids().unwrap()
    }
    v.pop().unwrap()
}

fn set_on_main_thread() {
   thread_local! {
       static IS_SET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
   }
   if !IS_SET.get() {
       IS_SET.set(true);
       core_id();
   }
}

fn mpmc(cap: Option<usize>) {
    let (tx, rx) = new(cap);

    crossbeam::scope(|scope| {
        for _ in 0..THREADS {
            let core_id = core_id();
            let tx = tx.clone();
            scope.spawn(move |_| {
                set_for_current(core_id);
                for i in 0..MESSAGES / THREADS {
                    tx.send(message::new(i)).unwrap();
                }
            });
        }

        for _ in 0..THREADS {
            let core_id = core_id();
            let rx = rx.clone();
            scope.spawn(move |_| {
            set_for_current(core_id);
                for _ in 0..MESSAGES / THREADS {
                    rx.recv().unwrap();
                }
            });
        }
    })
    .unwrap();
}

fn select_rx(cap: Option<usize>) {
    let chans = (0..THREADS).map(|_| new(cap)).collect::<Vec<_>>();

    crossbeam::scope(|scope| {
        for (tx, _) in &chans {
            let tx = tx.clone();
            let core_id = core_id();
            scope.spawn(move |_| {
            set_for_current(core_id);
                for i in 0..MESSAGES / THREADS {
                    tx.send(message::new(i)).unwrap();
                }
            });
        }

        for _ in 0..MESSAGES {
            let mut sel = Select::new();
            for (_, rx) in &chans {
                sel.recv(rx);
            }
            let case = sel.select();
            let index = case.index();
            case.recv(&chans[index].1).unwrap();
        }
    })
    .unwrap();
}

fn select_both(cap: Option<usize>) {
    let chans = std::sync::Arc::new((0..THREADS).map(|_| new(cap)).collect::<Vec<_>>());

    crossbeam::scope(|scope| {
        for _ in 0..THREADS {
            let core_id = core_id();
            let chans = chans.clone();
            scope.spawn(move |_| {
            set_for_current(core_id);
                for i in 0..MESSAGES / THREADS {
                    let mut sel = Select::new();
                    for (tx, _) in chans.iter() {
                        sel.send(tx);
                    }
                    let case = sel.select();
                    let index = case.index();
                    case.send(&chans[index].0, message::new(i)).unwrap();
                }
            });
        }

        for _ in 0..THREADS {
            let core_id = core_id();
            let chans = chans.clone();
            scope.spawn(move |_| {
            set_for_current(core_id);
                for _ in 0..MESSAGES / THREADS {
                    let mut sel = Select::new();
                    for (_, rx) in chans.iter() {
                        sel.recv(rx);
                    }
                    let case = sel.select();
                    let index = case.index();
                    case.recv(&chans[index].1).unwrap();
                }
            });
        }
    })
    .unwrap();
}

/*
fn main() {
    macro_rules! run {
        ($name:expr, $f:expr) => {
            let now = ::std::time::Instant::now();
            $f;
            let elapsed = now.elapsed();
            println!(
                "{:25} {:15} {:7.3} sec",
                $name,
                "Rust crossbeam-channel",
                elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1e9
            );
        };
    }

    /*
    run!("bounded0_mpmc", mpmc(Some(0)));
    run!("bounded0_mpsc", mpsc(Some(0)));
    run!("bounded0_select_both", select_both(Some(0)));
    run!("bounded0_select_rx", select_rx(Some(0)));
    run!("bounded0_spsc", spsc(Some(0)));

    run!("bounded1_mpmc", mpmc(Some(1)));
    run!("bounded1_mpsc", mpsc(Some(1)));
    run!("bounded1_select_both", select_both(Some(1)));
    run!("bounded1_select_rx", select_rx(Some(1)));
    run!("bounded1_spsc", spsc(Some(1)));

    run!("bounded_mpmc", mpmc(Some(MESSAGES)));
    run!("bounded_mpsc", mpsc(Some(MESSAGES)));
    run!("bounded_select_both", select_both(Some(MESSAGES)));
    run!("bounded_select_rx", select_rx(Some(MESSAGES)));
    run!("bounded_seq", seq(Some(MESSAGES)));
    run!("bounded_spsc", spsc(Some(MESSAGES)));
    */

    let core_id = core_id();
    set_for_current(core_id);
    run!("unbounded_mpmc", mpmc(None));
    run!("unbounded_mpsc", mpsc(None));
    run!("unbounded_spmc", spmc(None));
    run!("unbounded_select_both", select_both(None));
    run!("unbounded_select_rx", select_rx(None));
    run!("unbounded_seq", seq(None));
    run!("unbounded_spsc", spsc(None));
}
*/
fn bench_mpmc(bencher: &mut Criterion) {
    bencher.bench_function("mpmc", |b| { b.iter(|| {
        mpmc(None);
    }); });
}
fn bench_mpsc(bencher: &mut Criterion) {
    bencher.bench_function("mpsc", |b| { b.iter(|| {
        mpsc(None);
    }); });
}
fn bench_spmc(bencher: &mut Criterion) {
    bencher.bench_function("spmc", |b| { b.iter(|| {
        spmc(None);
    }); });
}
fn bench_select_both(bencher: &mut Criterion) {
    bencher.bench_function("select_both", |b| { b.iter(|| {
        select_both(None);
    }); });
}
fn bench_select_rx(bencher: &mut Criterion) {
    bencher.bench_function("select_rx", |b| { b.iter(|| {
        select_rx(None);
    }); });
}
fn bench_seq(bencher: &mut Criterion) {
    bencher.bench_function("seq", |b| { b.iter(|| {
        seq(None);
    }); });
}
fn bench_spsc(bencher: &mut Criterion) {
    bencher.bench_function("spsc", |b| { b.iter(|| {
        spsc(None);
    }); });
}


use criterion::{criterion_group, criterion_main, Criterion};
criterion_group!(benches, bench_mpmc, bench_mpsc, bench_spmc, bench_select_both, bench_select_rx, bench_seq, bench_spsc);
criterion_main!(benches);
