use std::collections::vec_deque::Iter;
use std::collections::{HashMap, VecDeque};

use tokio::sync::mpsc;
use tokio::task;

use crate::error::{JoshutoError, JoshutoErrorKind};
use crate::event::AppEvent;
use crate::io::{FileOperationProgress, IoWorkerObserver, IoWorkerThread};

pub struct WorkerContext {
    // forks of applications
    child_pool: HashMap<u32, task::JoinHandle<()>>,
    // to send info
    event_tx: mpsc::Sender<AppEvent>,
    // queue of IO workers
    worker_queue: VecDeque<IoWorkerThread>,
    // current worker
    worker: Option<IoWorkerObserver>,
}

impl WorkerContext {
    pub fn new(event_tx: mpsc::Sender<AppEvent>) -> Self {
        Self {
            child_pool: HashMap::new(),
            event_tx,
            worker_queue: VecDeque::new(),
            worker: None,
        }
    }
    pub fn clone_event_tx(&self) -> mpsc::Sender<AppEvent> {
        self.event_tx.clone()
    }
    // worker related
    pub async fn push_worker(&mut self, thread: IoWorkerThread) {
        self.worker_queue.push_back(thread);
        // error is ignored
        let _ = self.event_tx.send(AppEvent::IoWorkerCreate).await;
    }
    pub fn is_busy(&self) -> bool {
        self.worker.is_some()
    }
    pub fn is_empty(&self) -> bool {
        self.worker_queue.is_empty()
    }

    pub fn iter(&self) -> Iter<IoWorkerThread> {
        self.worker_queue.iter()
    }
    pub fn worker_ref(&self) -> Option<&IoWorkerObserver> {
        self.worker.as_ref()
    }

    pub fn set_progress(&mut self, res: FileOperationProgress) {
        if let Some(s) = self.worker.as_mut() {
            s.set_progress(res);
        }
    }

    pub fn get_msg(&self) -> Option<&str> {
        let worker = self.worker.as_ref()?;
        Some(worker.get_msg())
    }
    pub fn update_msg(&mut self) {
        if let Some(s) = self.worker.as_mut() {
            s.update_msg();
        }
    }

    pub fn start_next_job(&mut self) {
        let tx = self.clone_event_tx();

        if let Some(worker) = self.worker_queue.pop_front() {
            let src = worker.paths[0].parent().unwrap().to_path_buf();
            let dest = worker.dest.clone();
            let handle = task::spawn(async move {
                let (wtx, mut wrx) = mpsc::channel(1024);
                // start worker
                let worker_handle = task::spawn(async move { worker.start(wtx).await });
                // relay worker info to event loop
                while let Some(progress) = wrx.recv().await {
                    let _ = tx.send(AppEvent::FileOperationProgress(progress)).await;
                }
                let result = worker_handle.await;

                match result {
                    Ok(res) => {
                        let _ = tx.send(AppEvent::IoWorkerResult(res)).await;
                    }
                    Err(_) => {
                        let err = JoshutoError::new(
                            JoshutoErrorKind::UnknownError,
                            "Sending Error".to_string(),
                        );
                        let _ = tx.send(AppEvent::IoWorkerResult(Err(err))).await;
                    }
                }
            });
            let observer = IoWorkerObserver::new(handle, src, dest);
            self.worker = Some(observer);
        }
    }

    pub fn remove_worker(&mut self) -> Option<IoWorkerObserver> {
        self.worker.take()
    }

    pub fn push_child(&mut self, child_id: u32, handle: task::JoinHandle<()>) {
        self.child_pool.insert(child_id, handle);
    }

    pub async fn join_child(&mut self, child_id: u32) {
        if let Some(handle) = self.child_pool.remove(&child_id) {
            let _ = handle.await;
        }
    }
}
