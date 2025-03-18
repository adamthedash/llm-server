use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    sync::oneshot::{Receiver, Sender, channel},
    task::JoinHandle,
    time::sleep,
};
use tracing::info;
use uuid::Uuid;

pub struct Task {
    pub id: Uuid,
    prompt: String,
    output_channel: Sender<String>,
}

impl Task {
    pub fn new(prompt: String) -> (Self, Receiver<String>) {
        let (tx, rx) = channel();

        let id = Uuid::new_v4();

        let task = Self {
            id,
            prompt,
            output_channel: tx,
        };

        (task, rx)
    }
}

pub struct TaskProcessor {
    queue: Arc<Mutex<VecDeque<Task>>>,
    workers: Vec<JoinHandle<()>>,
}

async fn worker_process(queue: Arc<Mutex<VecDeque<Task>>>) {
    loop {
        let task = queue.lock().expect("Failed to lock queue").pop_front();
        if let Some(task) = task {
            // Mock some processing
            sleep(Duration::from_secs(1)).await;
            let response = format!("Ping {} -> Pong", task.prompt);

            if task.output_channel.send(response).is_err() {
                info!("Failed to send result, channel closed.");
            }
        } else {
            // Nothing in the queue
            sleep(Duration::from_millis(50)).await;
        }
    }
}

impl TaskProcessor {
    pub fn new(num_workers: usize) -> Self {
        let queue = Arc::new(Mutex::new(VecDeque::<Task>::new()));

        let workers = (0..num_workers)
            .map(|_| {
                let queue = queue.clone();
                tokio::spawn(async move { worker_process(queue).await })
            })
            .collect();

        Self { queue, workers }
    }

    pub fn submit_task(&mut self, task: Task) {
        self.queue
            .lock()
            .expect("Failed to lock queue")
            .push_back(task);
    }

    /// Cancells a task, returns true if the task was in the queue
    pub fn cancel_task(&mut self, task_id: &Uuid) -> bool {
        let mut queue = self.queue.lock().expect("Failed to lock queue");

        let index = queue.iter().position(|t| t.id == *task_id);
        if let Some(index) = index {
            queue.remove(index).expect("Index should exist");
            true
        } else {
            false
        }
    }
}

/// Guard which automatically cancels the task when it is terminated early
pub struct CancellationGuard {
    pub task_processor: Arc<Mutex<TaskProcessor>>,
    pub task_id: Uuid,
}

impl Drop for CancellationGuard {
    fn drop(&mut self) {
        let cancelled = self
            .task_processor
            .lock()
            .expect("Failed to lock task processor.")
            .cancel_task(&self.task_id);
        if cancelled {
            info!("Cancelling task: {}", self.task_id);
        }
    }
}

#[cfg(test)]
mod tests {

    use super::{Task, TaskProcessor};

    #[tokio::test]
    async fn test() {
        let mut worker_pool = TaskProcessor::new(4);
        let (task, rx) = Task::new("hello".to_string());

        worker_pool.submit_task(task);

        let val = rx.await.unwrap();
        print!("{}", val);
    }
}
