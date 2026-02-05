use std::sync::{Arc, Mutex};
use std::thread;
use rayon::prelude::*;


struct Task {
    name: String,
    action : Box<dyn Fn() + Send + 'static>,
}

struct TaskManager {
    tasks: Arc<Mutex<Vec<Task>>>,
}

impl TaskManager {
    pub fn new() -> Self {
        TaskManager {
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_task<F>(&self, name: String, action: F)
    where
        F: Fn() + Send + 'static,
    {
        let task = Task {
            name,
            action: Box::new(action),
        };
        let mut tasks = self.tasks.lock().unwrap();
        tasks.push(task);
    }

    pub fn run_tasks(&self) {
        let tasks = self.tasks.lock().unwrap();
        tasks.par_iter().for_each(|task| {
            (task.action)();
        });
    }
}



