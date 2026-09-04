use hir::{LocalStorage, StorageBackend};

fn main() {
    let storage = LocalStorage::new(String::from("value"));
    let escaped = storage.inspect(String::as_str);
    drop(storage);
    drop(escaped);
}
