use pdma_model_contract::bridge::{
    BridgeError, encode_id_value, encode_memory_address_value, encode_semantic_event,
};
use pdma_model_contract::semantic::{BufferId, CommandId, EventErrorKind, EventRecord, QueueId};
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
struct AccessSentinel {
    accesses: AtomicU64,
}

impl AccessSentinel {
    fn record_access(&self) {
        self.accesses.fetch_add(1, Ordering::SeqCst);
    }

    fn observed(&self) -> u64 {
        self.accesses.load(Ordering::SeqCst)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let destination = [0xa5_u8; 64];
    let store = AccessSentinel::default();
    let _: fn(u128) -> Result<_, BridgeError> = encode_id_value;
    let _: fn(u128) -> Result<_, BridgeError> = encode_memory_address_value;
    let id =
        encode_id_value(u128::from(u64::MAX) + 1).expect_err("overflowing ID must be rejected");
    let address = encode_memory_address_value(u128::from(u64::MAX) + 1)
        .expect_err("overflowing address must be rejected");
    let event = EventRecord::new(
        QueueId::new(1),
        CommandId::new(2),
        EventErrorKind::UnknownBuffer,
        BufferId::new(3),
    );
    let locator = encode_semantic_event(event).expect_err("missing locator must be rejected");
    let _store_access: fn(&AccessSentinel) = AccessSentinel::record_access;
    assert_eq!(
        store.observed(),
        0,
        "pure bridge touched the store sentinel"
    );
    println!("id={id:?};source={}", source_name(&id));
    println!("address={address:?};source={}", source_name(&address));
    println!("locator={locator:?};source={}", source_name(&locator));
    println!(
        "unchanged={};store-accesses={}",
        destination == [0xa5_u8; 64],
        store.observed()
    );
    Ok(())
}

fn source_name(error: &BridgeError) -> &'static str {
    let Some(source) = error.source() else {
        return "none";
    };
    if source
        .downcast_ref::<pdma_model_contract::abi::AbiEncodeError>()
        .is_some()
    {
        "AbiEncodeError"
    } else if source
        .downcast_ref::<pdma_model_contract::abi::AbiAddressError>()
        .is_some()
    {
        "AbiAddressError"
    } else {
        "unexpected"
    }
}
