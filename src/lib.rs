use std::error::Error;
use std::num::NonZeroUsize;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use chrono::offset::Local;

pub fn log(s: &str, is_err: bool, jobnum: usize) {
    let time = Local::now().format("%d/%m/%y %H:%M:%S");

    let jobnum = match jobnum {
        0 => "Main".to_owned(),
        other => format!("{other}"),
    };

    match is_err {
        false => println!("[II] - {time} - {jobnum} - {s}"),
        true => eprintln!("[EE] - {time} - {jobnum} - {s}"),
    }
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    tx: mpsc::Sender<Message>,
}

type Job = (Box<dyn FnOnce() + Send + 'static>, usize);

enum Message {
    NewJob(Job),
    Terminate,
}

impl ThreadPool {
    pub fn new(size: NonZeroUsize) -> Self {
        let mut workers = Vec::with_capacity(size.get());

        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));

        for id in 0..size.get() {
            workers.push(Worker::new(id, Arc::clone(&rx)));
        }

        Self { workers, tx }
    }

    // TODO: Better error handling
    pub fn execute<F>(&self, f: F, jobnum: &usize) -> Result<(), Box<dyn Error>>
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.tx.send(Message::NewJob((job, *jobnum)))?;

        Ok(())
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        log("Terminating all workers", false, 0);

        for _ in &self.workers {
            self.tx.send(Message::Terminate).unwrap();
        }

        log("Shutting down all workers", false, 0);

        for worker in &mut self.workers {
            let log_msg = format!("Shutting down worker {}", worker.id);
            log(&log_msg, false, 0);

            if let Some(join_handle) = worker.join_handle.take() {
                join_handle.join().unwrap()
            }
        }
    }
}

struct Worker {
    id: usize,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl Worker {
    pub fn new(id: usize, rx: Arc<Mutex<mpsc::Receiver<Message>>>) -> Self {
        let join_handle = thread::spawn(move || loop {
            let message = rx
                .lock()
                .expect("another thread panicked while holding the lock")
                .recv()
                .expect("the thread pool has been dropped");

            match message {
                Message::NewJob((job, jobnum)) => {
                    let log_msg = format!("Worker {id} is executing a job");
                    log(&log_msg, false, jobnum);
                    job();
                }
                Message::Terminate => {
                    break;
                }
            }
        });

        Self {
            id,
            join_handle: Some(join_handle),
        }
    }
}
