use hir::{LocalStorage, StorageBackend};

fn main() {
    let storage = LocalStorage::new(1_u64);
    let future = storage.mutate(async |value| *value += 1);
    drop(storage);
    drop(future);
}
