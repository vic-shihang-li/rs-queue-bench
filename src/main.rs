use core::sync::atomic::{AtomicUsize, Ordering};
use lockfree::channel::spsc;
use nolock::queues::spsc::unbounded;
use rtrb::{PushError, RingBuffer};
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
    let start_gate = Arc::new(Barrier::new(3));
    let end_gate = Arc::new(Barrier::new(3));

    let t1_start = start_gate.clone();
    let t1_end = end_gate.clone();
    let t1 = thread::spawn(move || {
        t1_start.wait();
        for i in 0..num_inserts {
            match tx.send(1) {
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
        assert_eq!(_sum, num_inserts);
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

fn bench_nolock(num_inserts: usize) -> usize {
    let (mut rx, mut tx) = unbounded::queue::<usize>();
    let start_gate = Arc::new(Barrier::new(3));
    let end_gate = Arc::new(Barrier::new(3));

    let t1_start = start_gate.clone();
    let t1_end = end_gate.clone();
    let t1 = thread::spawn(move || {
        t1_start.wait();
        for i in 0..num_inserts {
            match tx.enqueue(1) {
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
        assert_eq!(_sum, num_inserts);
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

fn bench_lockfree(num_inserts: usize) -> usize {
    let (mut tx, mut rx) = spsc::create::<usize>();
    let start_gate = Arc::new(Barrier::new(3));
    let end_gate = Arc::new(Barrier::new(3));

    let t1_start = start_gate.clone();
    let t1_end = end_gate.clone();
    let t1 = thread::spawn(move || {
        t1_start.wait();
        for i in 0..num_inserts {
            match tx.send(1) {
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
        assert_eq!(_sum, num_inserts);
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

fn bench_rtrb(num_inserts: usize) -> usize {
    let (mut tx, mut rx) = RingBuffer::new((num_inserts / 3) as usize);
    let start_gate = Arc::new(Barrier::new(3));
    let end_gate = Arc::new(Barrier::new(3));

    let t1_start = start_gate.clone();
    let t1_end = end_gate.clone();
    let t1 = thread::spawn(move || {
        t1_start.wait();
        let mut ctr = 0;
        for _ in 0..num_inserts {
            'inner: loop {
                match tx.push(1) {
                    Ok(_) => {
                        ctr += 1;
                        break 'inner;
                    }
                    Err(PushError::Full(_)) => (),
                }
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

        loop {
            match rx.pop() {
                Err(_) => {}
                Ok(i) => _sum += i,
            }
            if _sum == num_inserts {
                break;
            }
        }
        assert_eq!(_sum, num_inserts);
        t2_end.wait();
        // println!("{}", _sum);
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
    const NUM_INSERTS: usize = 100_000;

    let std_mpsc_bench = make_bench!(bench_std_mpsc, NUM_INSERTS);
    let nolock_bench = make_bench!(bench_nolock, NUM_INSERTS);
    let lockfree_bench = make_bench!(bench_lockfree, NUM_INSERTS);
    let rtrb_bench = make_bench!(bench_rtrb, NUM_INSERTS);

    bench("std::mpsc", std_mpsc_bench, NUM_TRIALS);
    bench("nolock", nolock_bench, NUM_TRIALS);
    bench("lockfree", lockfree_bench, NUM_TRIALS);
    bench("rtrb", rtrb_bench, NUM_TRIALS);
}
