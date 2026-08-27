use pdma_model_device::{
    NoopRegisterObserver, RegisterObservation, RegisterObserver, SharedMemory,
};

fn main() {
    let _memory = SharedMemory::new();
    let observer = NoopRegisterObserver;
    let _: &dyn RegisterObserver = &observer;
    let _: Option<RegisterObservation> = None;
}
