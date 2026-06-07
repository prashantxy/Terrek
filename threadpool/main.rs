use std::sync::{mpsc,Arc,Mutex};
use std::thread;



// A Job is a boxed closure that can be sent across threads
// FnOnce: can be called once
// Send: can be transferred between threads
// 'static: lives for the entire program duration
type Job = Box<dyn FnOnce() + Send + 'static>;

// Worker holds a thread handle and an identifier
struct Worker{
    id : usize;
    thread : Option<thread::JoinHandle<()>>;
}

pub struct ThreadPoolBuilder{
    size: usize,
    name_prefix: String,
    stack_size: Option<usize>,
}

impl soundsgood(){
      let threadpool  
}
fn main(){
    let pool = ThreadPool::new(5);
     
    for i in 0..30{
        pool.execute(move || {
            println!("Task {} running on thread {:?}", i, thread::current().id());
            thread::sleep(std::time::Duration::from_millis(100));
            println!("Task {} complete", i);
        })
    }
    // Pool is dropped here, triggering graceful shutdown
    // All tasks complete before main exits
}