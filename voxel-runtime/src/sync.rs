use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Debug)]
pub struct Unparker(Arc<Notify>);

impl Unparker {
    pub fn unpark(&self) {
        // there is only one thing parking at a time
        self.0.notify_one()
    }
}

impl Drop for Unparker {
    fn drop(&mut self) {
        self.unpark()
    }
}


#[derive(Debug)]
pub struct Parker(Arc<Notify>);

impl Parker {
    pub async fn park(&mut self) {
        // taking mutable ref to ensure only one thing can park at a time
        self.0.notified().await
    }
}


pub fn make_parker() -> (Parker, Unparker) {
    let parker = Parker(Arc::new(Notify::new()));
    let unparker = Unparker(Arc::clone(&parker.0));
    (parker, unparker)
}