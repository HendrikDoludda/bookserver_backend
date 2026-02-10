use std::sync::{Arc};
use tokio::sync::{mpsc,Semaphore};


type Task = Box<dyn FnOnce() ->tokio::task::JoinHandle<()> + Send + 'static>;


#[derive(Clone)]
pub struct TaskManager {
    sender: mpsc::Sender<Task>,
}

impl TaskManager {
    

    pub fn new(max_workers: usize, queue_size: usize) -> Self {
        let (sender, mut receiver) = mpsc::channel::<Task>(queue_size);
        let semaphore = Arc::new(Semaphore::new(max_workers));
        tokio::spawn({
            let semaphore = semaphore.clone();
            async move {
                while let Some(task) = receiver.recv().await {
                    let permit = semaphore.clone().acquire_owned().await.unwrap();
                    let handle = task();
                    tokio::spawn(async move {
                        let _permit = permit;
                        handle.await.ok();
                    });
                }
            }
        });
        Self {sender}
    }

    pub async fn add_task<F, Fut>(&self, task: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let wrapped: Task = Box::new(|| {
            tokio::spawn(task())
        });
        self.sender.send(wrapped).await.unwrap();
    }
}



