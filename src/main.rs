use core::sync::atomic::{AtomicUsize, Ordering};
use lockfree::channel::spsc;
use nolock::queues::spsc::unbounded;
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

fn bench_nolock(num_inserts: i32) -> usize {
    let (mut rx, mut tx) = unbounded::queue::<i32>();
    let start_gate = Arc::new(Barrier::new(3));
    let end_gate = Arc::new(Barrier::new(3));

    let t1_start = start_gate.clone();
    let t1_end = end_gate.clone();
    let t1 = thread::spawn(move || {
        t1_start.wait();
        for i in 0..num_inserts {
            match tx.enqueue(i) {
                Err(e) => println!("{:?}", e), // should not err here
                Ok(_) => (),
            }
        }
        drop(tx);
        t1_end.wait();
    });

    let t2_start = start_gate.clone();
    let t2_end = end_gate.clone();
    let t2 = thread::spawn(move || {
        t2_start.wait();
        let mut _sum = 0;
        while let Ok(val) = rx.try_dequeue() {
            _sum += val;
        }
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

fn bench_lockfree(num_inserts: i32) -> usize {
    let (mut tx, mut rx) = spsc::create::<i32>();
    let start_gate = Arc::new(Barrier::new(3));
    let end_gate = Arc::new(Barrier::new(3));

    let t1_start = start_gate.clone();
    let t1_end = end_gate.clone();
    let t1 = thread::spawn(move || {
        t1_start.wait();
        for i in 0..num_inserts {
            match tx.send(i) {
                Err(e) => println!("{:?}", e), // should not err here
                Ok(_) => (),
            }
        }
        drop(tx);
        t1_end.wait();
    });

    let t2_start = start_gate.clone();
    let t2_end = end_gate.clone();
    let t2 = thread::spawn(move || {
        t2_start.wait();
        let mut _sum = 0;
        while let Ok(val) = rx.recv() {
            _sum += val;
        }
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

fn repeat(ntimes: i32, procedure: BenchProcedure) -> usize {
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
    let sum = repeat(num_trials, benchmark);
    let avg: f64 = (sum as f64) / (num_trials as f64);

    println!(
        "Bench '{name}': trials: {trials}; average (millis): {avg}",
        name = name,
        trials = num_trials,
        avg = avg
    );
}

fn main() {
    const NUM_TRIALS: i32 = 1_000;
    const NUM_INSERTS: i32 = 100_000;

    let nolock_bench = make_bench!(bench_nolock, NUM_INSERTS);
    let lockfree_bench = make_bench!(bench_lockfree, NUM_INSERTS);

    bench("nolock", nolock_bench, NUM_TRIALS);
    bench("lockfree", lockfree_bench, NUM_TRIALS);
}
