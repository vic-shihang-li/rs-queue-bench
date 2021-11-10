mod utils;
use crossbeam_queue::{ArrayQueue, SegQueue};
use std::{
    sync::{Arc, Barrier},
    thread::{self, JoinHandle},
    time::Instant,
};
use threadpool::ThreadPool;

fn bench_crossbeam_bounded_queue(
    n_producers: usize,
    n_consumers: usize,
    n_inserts_per_thr: usize,
    queue_sz: usize,
) -> usize {
    let q = Arc::new(ArrayQueue::<usize>::new(queue_sz));

    let start_gate = Arc::new(Barrier::new(n_producers + n_consumers + 1));
    let end_gate = Arc::new(Barrier::new(n_producers + n_consumers + 1));

    let producer_pool = ThreadPool::new(n_producers);
    for _ in 0..n_producers {
        let prod_start = start_gate.clone();
        let prod_end = end_gate.clone();
        let q_enqueuer = q.clone();
        producer_pool.execute(move || {
            prod_start.wait();

            let mut ctr = 0;
            loop {
                if q_enqueuer.push(1).is_ok() {
                    ctr += 1;
                    if ctr == n_inserts_per_thr {
                        break;
                    }
                }
            }

            prod_end.wait();
        })
    }

    let deq_per_thr = (n_inserts_per_thr * n_producers) / n_consumers;
    let consumer_pool = ThreadPool::new(n_consumers);
    for _ in 0..n_consumers {
        let cons_start = start_gate.clone();
        let cons_end = end_gate.clone();
        let q_dequeuer = q.clone();
        consumer_pool.execute(move || {
            cons_start.wait();

            let mut ctr = 0;
            loop {
                if q_dequeuer.pop().is_some() {
                    ctr += 1;
                    if ctr == deq_per_thr {
                        break;
                    }
                }
            }

            cons_end.wait();
        })
    }

    start_gate.wait();
    let start = Instant::now();
    end_gate.wait();
    let dur = start.elapsed();

    producer_pool.join();
    consumer_pool.join();

    dur.as_millis() as usize
}

fn bench_crossbeam_unbounded_queue(
    n_producers: usize,
    n_consumers: usize,
    n_inserts_per_thr: usize,
) -> usize {
    let q = Arc::new(SegQueue::<usize>::new());

    let start_gate = Arc::new(Barrier::new(n_producers + n_consumers + 1));
    let end_gate = Arc::new(Barrier::new(n_producers + n_consumers + 1));

    let producer_pool = ThreadPool::new(n_producers);
    for _ in 0..n_producers {
        let prod_start = start_gate.clone();
        let prod_end = end_gate.clone();
        let q_enqueuer = q.clone();
        producer_pool.execute(move || {
            prod_start.wait();

            for _ in 0..n_inserts_per_thr {
                q_enqueuer.push(1);
            }

            prod_end.wait();
        })
    }

    let deq_per_thr = (n_inserts_per_thr * n_producers) / n_consumers;
    let consumer_pool = ThreadPool::new(n_consumers);
    for _ in 0..n_consumers {
        let cons_start = start_gate.clone();
        let cons_end = end_gate.clone();
        let q_dequeuer = q.clone();
        consumer_pool.execute(move || {
            cons_start.wait();

            let mut ctr = 0;
            loop {
                if q_dequeuer.pop().is_some() {
                    ctr += 1;
                    if ctr == deq_per_thr {
                        break;
                    }
                }
            }

            cons_end.wait();
        })
    }

    start_gate.wait();
    let start = Instant::now();
    end_gate.wait();
    let dur = start.elapsed();

    producer_pool.join();
    consumer_pool.join();

    dur.as_millis() as usize
}

trait QueueBench {
    fn producer_count(&self) -> usize;
    fn consumer_count(&self) -> usize;
    fn producer(&self) -> std::thread::JoinHandle<()>;
    fn consumer(&self) -> std::thread::JoinHandle<()>;
}

struct CrossbeamUnboundedQueueBench {
    queue: Arc<SegQueue<usize>>,
    n_producers: usize,
    n_consumers: usize,
    inserts_per_thr: usize,
}

impl CrossbeamUnboundedQueueBench {
    fn new(n_producers: usize, n_consumers: usize, inserts_per_thr: usize) -> Self {
        CrossbeamUnboundedQueueBench {
            queue: Arc::new(SegQueue::<usize>::new()),
            n_producers,
            n_consumers,
            inserts_per_thr,
        }
    }
}

impl QueueBench for CrossbeamUnboundedQueueBench {
    fn producer_count(&self) -> usize {
        self.n_producers
    }

    fn consumer_count(&self) -> usize {
        self.n_consumers
    }

    fn producer(&self) -> std::thread::JoinHandle<()> {
        let q_enqueuer = self.queue.clone();
        let inserts_per_thr = self.inserts_per_thr;

        thread::spawn(move || {
            for _ in 0..inserts_per_thr {
                q_enqueuer.push(1);
            }
        })
    }

    fn consumer(&self) -> std::thread::JoinHandle<()> {
        let q_dequeuer = self.queue.clone();
        let deq_per_thr = (self.inserts_per_thr * self.n_producers) / self.n_consumers;

        thread::spawn(move || {
            let mut ctr = 0;
            loop {
                if q_dequeuer.pop().is_some() {
                    ctr += 1;
                    if ctr == deq_per_thr {
                        break;
                    }
                }
            }
        })
    }
}

fn bench_mpsc<T: QueueBench>(benchmark: T) -> usize {
    let waiter_count = benchmark.producer_count() + benchmark.consumer_count() + 1;
    let start_gate = Arc::new(Barrier::new(waiter_count));
    let end_gate = Arc::new(Barrier::new(waiter_count));

    // TODO: figure out how to insert start and end on each worker thread
    let mut producers: Vec<JoinHandle<()>> = Vec::with_capacity(benchmark.producer_count());
    (0..benchmark.producer_count()).for_each(|i| {
        producers[i] = benchmark.producer();
    });

    let mut consumers: Vec<JoinHandle<()>> = Vec::with_capacity(benchmark.consumer_count());
    (0..benchmark.consumer_count()).for_each(|i| {
        consumers[i] = benchmark.consumer();
    });

    start_gate.wait();
    let start = Instant::now();
    end_gate.wait();
    let dur = start.elapsed();

    dur.as_millis() as usize
}

fn main() {
    use utils::bench;

    const INSERTS_PER_THR: usize = 10_000;
    const N_CONSUMERS: usize = 10;
    const N_PRODUCERS: usize = 10;
    const QUEUE_SZ: usize = INSERTS_PER_THR * N_PRODUCERS;
    const N_TRIALS: u32 = 1_000;

    bench(
        "crossbeam_unbounded",
        || bench_crossbeam_unbounded_queue(N_PRODUCERS, N_CONSUMERS, INSERTS_PER_THR),
        N_TRIALS,
    );

    bench(
        "crossbeam_bounded",
        || bench_crossbeam_bounded_queue(N_PRODUCERS, N_CONSUMERS, INSERTS_PER_THR, QUEUE_SZ),
        N_TRIALS,
    )
}
