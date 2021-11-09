use core::sync::atomic::{AtomicUsize, Ordering};
use lockfree::channel::spsc;
use nolock::queues::spsc::unbounded;
use rtrb::RingBuffer;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;
use threadpool::ThreadPool;

type BenchProcedure = fn() -> usize;

/// Define the number of inserts to perform on.
macro_rules! make_bench {
    ($bench_fn: ident, $num_inserts:expr) => {
        || $bench_fn($num_inserts)
    };
}

fn bench_std_mpsc(num_inserts: usize) -> usize {
    let (tx, rx) = std::sync::mpsc::channel::<usize>();

    bench_spsc(
        move || {
            for _ in 0..num_inserts {
                match tx.send(1) {
                    Err(e) => println!("{:?}", e),
                    Ok(_) => (),
                };
            }
            drop(tx)
        },
        move || {
            let mut sum = 0;
            loop {
                match rx.recv() {
                    Err(_) => {}
                    Ok(i) => sum += i,
                }
                if sum == num_inserts {
                    break;
                }
            }
            assert_eq!(sum, num_inserts);
        },
    )
}

#[allow(dead_code)]
fn bench_crate_nolock(num_inserts: usize) -> usize {
    let (mut rx, mut tx) = unbounded::queue::<usize>();

    bench_spsc(
        move || {
            for i in 0..num_inserts {
                match tx.enqueue(i) {
                    Err(e) => println!("{:?}", e),
                    Ok(_) => (),
                };
            }
            drop(tx)
        },
        move || {
            let mut ctr = 0;
            loop {
                match rx.try_dequeue() {
                    Err(_) => {}
                    Ok(_) => ctr += 1,
                };
                if ctr == num_inserts {
                    break;
                }
            }
            assert_eq!(ctr, num_inserts);
        },
    )
}

fn bench_crate_lockfree(num_inserts: usize) -> usize {
    let (mut tx, mut rx) = spsc::create::<usize>();

    bench_spsc(
        move || {
            for _ in 0..num_inserts {
                match tx.send(1) {
                    Err(e) => println!("{:?}", e),
                    Ok(_) => (),
                };
            }
            drop(tx)
        },
        move || {
            let mut recv_ctr = 0;
            loop {
                match rx.recv() {
                    Err(_) => {}
                    Ok(_) => recv_ctr += 1,
                }
                if recv_ctr == num_inserts {
                    break;
                }
            }
            assert_eq!(recv_ctr, num_inserts);
        },
    )
}

fn bench_crate_rtrb(num_inserts: usize) -> usize {
    let (mut tx, mut rx) = RingBuffer::new(num_inserts as usize);

    bench_spsc(
        move || {
            for i in 0..num_inserts {
                match tx.push(i) {
                    Err(e) => println!("{:?}", e),
                    Ok(_) => (),
                };
            }
            drop(tx)
        },
        move || {
            let mut recv_ctr = 0;
            loop {
                match rx.pop() {
                    Err(_) => {}
                    Ok(_) => recv_ctr += 1,
                }
                if recv_ctr == num_inserts {
                    break;
                }
            }
            assert_eq!(recv_ctr, num_inserts);
        },
    )
}

/// Measures how long a single-producer single-consumer workflow completes.
///
/// Specifically, measures the elapsed time between two threads (a producer
/// and a consumer) beginning their work, and the time when both have completed.
/// All work for producer and consumer are encapsulated in the two closures that
/// this function accepts.
fn bench_spsc(
    producer: impl FnOnce() + Send + 'static,
    consumer: impl FnOnce() + Send + 'static,
) -> usize {
    let start_gate = Arc::new(Barrier::new(3));
    let end_gate = Arc::new(Barrier::new(3));

    let t1_start = start_gate.clone();
    let t1_end = end_gate.clone();
    let t1 = thread::spawn(move || {
        t1_start.wait();
        producer();
        t1_end.wait();
    });

    let t2_start = start_gate.clone();
    let t2_end = end_gate.clone();
    let t2 = thread::spawn(move || {
        t2_start.wait();
        consumer();
        t2_end.wait();
    });

    start_gate.wait();
    let start = Instant::now();
    end_gate.wait();
    let dur = start.elapsed();

    t1.join().unwrap();
    t2.join().unwrap();

    dur.as_millis() as usize
}

/// Repeatedly invokes a function that generates a number, for N times.
/// Returns the sum of every number generated by the procedure.
fn repeat_sum(ntimes: i32, procedure: BenchProcedure) -> usize {
    // use worker threads to do repeated benchmark runs
    let concurrent_test_thread_count = 5;
    let pool = ThreadPool::new(concurrent_test_thread_count);

    let sum = Arc::new(AtomicUsize::new(0));
    for _ in 0..ntimes {
        let sum_clone = sum.clone();
        pool.execute(move || {
            sum_clone.fetch_add(procedure(), Ordering::SeqCst);
        });
    }
    pool.join();

    sum.load(Ordering::SeqCst)
}

fn bench(name: &'static str, benchmark: BenchProcedure, num_trials: i32) {
    let sum = repeat_sum(num_trials, benchmark);
    let avg: f64 = (sum as f64) / (num_trials as f64);

    println!(
        "Bench '{name}': trials: {trials}; average (millis): {avg}",
        name = name,
        trials = num_trials,
        avg = avg
    );
}

fn main() {
    const NUM_TRIALS: i32 = 1000;
    const NUM_INSERTS: usize = 1_000_000;

    let std_mpsc_bench = make_bench!(bench_std_mpsc, NUM_INSERTS);
    // let nolock_bench = make_bench!(bench_crate_nolock, NUM_INSERTS);
    let lockfree_bench = make_bench!(bench_crate_lockfree, NUM_INSERTS);
    let rtrb_bench = make_bench!(bench_crate_rtrb, NUM_INSERTS);

    bench("std::sync::mpsc", std_mpsc_bench, NUM_TRIALS);
    // bench("nolock", nolock_bench, NUM_TRIALS);
    bench("lockfree", lockfree_bench, NUM_TRIALS);
    bench("rtrb", rtrb_bench, NUM_TRIALS);
}
