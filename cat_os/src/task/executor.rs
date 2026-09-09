use super::{Task, TaskId};
use alloc::task::Wake;
use alloc::{collections::BTreeMap, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;

pub const MAX_TASKS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    TaskLimitReached,
    DuplicateTaskId,
}

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Arc<TaskWaker>>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(MAX_TASKS)),
            waker_cache: BTreeMap::new(),
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn spawn(&mut self, task: Task) -> Result<(), SpawnError> {
        let task_id = task.id;
        if self.tasks.len() >= MAX_TASKS {
            return Err(SpawnError::TaskLimitReached);
        }
        if self.tasks.contains_key(&task_id) {
            return Err(SpawnError::DuplicateTaskId);
        }
        if self.task_queue.push(task_id).is_err() {
            return Err(SpawnError::TaskLimitReached);
        }
        self.tasks.insert(task_id, task);
        Ok(())
    }
}

impl Executor {
    fn run_ready_tasks(&mut self) {
        // destructure `self` to avoid borrow checker errors
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // task no longer exists
            };
            let task_waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| Arc::new(TaskWaker::new(task_id, task_queue.clone())))
                .clone();
            task_waker.mark_dequeued();
            let waker = Waker::from(task_waker.clone());
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    task_waker.deactivate();
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

    /// Polls all tasks that are currently ready and then returns.
    pub fn run_until_stalled(&mut self) {
        self.run_ready_tasks();
    }
}

impl Executor {
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};

        interrupts::disable();
        if self.task_queue.is_empty() {
            enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
    queued: AtomicBool,
    active: AtomicBool,
}

impl TaskWaker {
    fn wake_task(&self) {
        if !self.active.load(Ordering::Acquire) || self.queued.swap(true, Ordering::AcqRel) {
            return;
        }

        if !self.active.load(Ordering::Acquire) {
            self.queued.store(false, Ordering::Release);
            return;
        }

        if self.task_queue.push(self.task_id).is_err() {
            // The executor limits active tasks and coalesces wakes, so this can
            // only occur if the invariants are violated. Keep the task wakeable
            // instead of panicking in interrupt context.
            self.queued.store(false, Ordering::Release);
        }
    }

    fn mark_dequeued(&self) {
        self.queued.store(false, Ordering::Release);
    }

    fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
        self.queued.store(false, Ordering::Release);
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Self {
        TaskWaker {
            task_id,
            task_queue,
            queued: AtomicBool::new(false),
            active: AtomicBool::new(true),
        }
    }
}
