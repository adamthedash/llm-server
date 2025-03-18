/*!
Basic web server to learn about Axum

The server will serve as an LLM assistant. It can take requests from multiple clients simultaneously.
The server will manage a pool of workers which will take a prompt and return a response.
When a client sends a request, it will hold an open connection to the server until either:
    - The response is completed
    - The client closes the connection
When a connection is closed by the client, the request is immediately stopped processing so that resources can be freed up for another client.


From the outside:
    - POST /submit:
        Accepts a JSON data: {"prompt": "..."}
        Returns a JSON data: {"response": "..."}


First iteration:
    - No cancelling stuff
    - Single request at a time, blocking
    - Single worker


Resources:
- Generic connection pool crate: https://docs.rs/bb8/0.8.1/bb8/index.html
- Looks like with ONNX, we can have multiple threads run with the same model graph at once? https://onnxruntime.ai/docs/reference/high-level-design.html#key-design-decisions

Handling dropped connections / cancelled requests:
    Option 1) Poll the connection every few ms to see if it's closed
    Option 2) Hook axum's internal event system to get a notification when the connection is dropped.
        - Use tokio::select! to wait for either request completion or connection drop
    Option 3) Axum might just natively handle this and I just implement Drop for cleanup? https://github.com/tokio-rs/axum/discussions/1094

New design:
- We maintain a queue of tasks, each with an ID
- When a request comes in, we add a new task to the queue
- When a request is dropped, we remove it from the queue if it hasn't been taken for processing yet
- The worker pool consumes tasks in the queue, processes them, and adds the result to an output queue
    - Or do we use a channel and send it down there?

*/

pub mod task;

use std::sync::{Arc, Mutex};

use axum::extract::{Json, State};
use axum::{Router, routing::post};
use serde::Deserialize;
use serde_json::{Value, json};
use task::{CancellationGuard, Task, TaskProcessor};
use tokio::net::TcpListener;
use tracing::info;

#[derive(Deserialize)]
struct Prompt {
    prompt: String,
}

/// Submit a prompt to be processed by the LLM
async fn post_submit(
    State(workers): State<Arc<Mutex<TaskProcessor>>>,
    Json(payload): Json<Prompt>,
) -> Json<Value> {
    info!("Recieved request");

    let (task, rx) = Task::new(payload.prompt);
    let _guard = CancellationGuard {
        task_id: task.id,
        task_processor: workers.clone(),
    };
    workers
        .lock()
        .expect("Failed to lock worker pool")
        .submit_task(task);

    let response = rx.await.expect("Error receiving result");

    info!("Finished request");

    Json(json!({"response": response}))
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    tracing_subscriber::fmt::init();

    let worker_pool = Arc::new(Mutex::new(TaskProcessor::new(2)));

    let app = Router::new()
        .route("/submit", post(post_submit))
        .with_state(worker_pool);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
