use core::sync::atomic::{AtomicUsize, Ordering};
use nolock::queues::spsc::unbounded;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;
use threadpool::ThreadPool;

fn nolock_benchmark() -> u128 {
    let num_inserts = 100_000;

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

    dur.as_millis()
}

fn repeat(ntimes: i32, procedure: fn() -> u128) -> usize {
    // use worker threads to do repeated benchmark runs
    let concurrent_test_thread_count = 5;
    let pool = ThreadPool::new(concurrent_test_thread_count);

    let sum = Arc::new(AtomicUsize::new(0));
    for _ in 0..ntimes {
        let sum_clone = sum.clone();
        pool.execute(move || {
            sum_clone.fetch_add(procedure() as usize, Ordering::SeqCst);
        });
    }
    pool.join();

    sum.load(Ordering::SeqCst)
}

fn bench(name: &'static str, benchmark: fn() -> u128, num_trials: i32) {
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
    let num_trials = 1_000;
    bench("nolock", nolock_benchmark, num_trials);
}
