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


*/

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Json, State};
use axum::{routing::post, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio::time::sleep;
use tracing::info;

/// Pool of LLM workers
struct WorkerPool {
    workers: Vec<()>,
    permits: Semaphore,
}

impl WorkerPool {
    fn new(num_workers: usize) -> Self {
        Self {
            workers: vec![(); num_workers],
            permits: Semaphore::new(num_workers),
        }
    }

    /// Predict with the LLM
    async fn predict(&self, prompt: &str) -> String {
        // Semaphore so we can have N concurrent tasks
        let _permit = self
            .permits
            .acquire()
            .await
            .expect("Sempahore has been closed.");

        // Simulate some processing
        sleep(Duration::from_secs(1)).await;
        let response = format!("Ping {:?} -> Pong\n", prompt);

        response
    }
}

#[derive(Deserialize)]
struct Prompt {
    prompt: String,
}

/// Submit a prompt to be processed by the LLM
async fn post_submit(
    State(workers): State<Arc<WorkerPool>>,
    Json(payload): Json<Prompt>,
) -> Json<Value> {
    info!("Recieved request");
    let response = workers.predict(&payload.prompt).await;
    info!("Finished request");

    Json(json!({"response": response}))
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    tracing_subscriber::fmt::init();

    let workers = Arc::new(WorkerPool::new(2));

    let app = Router::new()
        .route("/submit", post(post_submit))
        .with_state(workers);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
