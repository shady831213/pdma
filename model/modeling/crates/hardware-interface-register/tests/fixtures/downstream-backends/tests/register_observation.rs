#![allow(missing_docs, reason = "external fixture test")]

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use hardware_interface_register::{
    AccessOrigin, BlockingObservationBackend, LocalObservationBackend, RegisterBus,
    RegisterBusAddress, RegisterBusBase, RegisterImage, RegisterObservation, RegisterObserver,
    SharedRegisterObserver,
};
use hardware_interface_storage::LocalStorage;
use his::{register_map, register_schema};

register_schema! {
    pub register ConsumerControl {
        width: 2, alignment: 2, endian: Little, access: ReadWrite,
        reset: Constant(0xa103), writable_mask: 0xff0f,
        fields: { enable: 0..1, mode: 1..4, tag: 8..16 },
    }
}

register_map! {
    pub map ConsumerRegisters {
        alignment: 8,
        extent: 16,
        placements: [control: ConsumerControl { offset: 8 }],
        access: [queues.control => control],
    }
}

#[derive(Debug, Default)]
struct Recorder(Mutex<Vec<RegisterObservation>>);

impl RegisterObserver for Recorder {
    fn observe(&self, observation: RegisterObservation) {
        lock(&self.0).push(observation);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn local_observations() -> Result<Vec<RegisterObservation>, Box<dyn std::error::Error>> {
    let recorder = Arc::new(Recorder::default());
    let backend = LocalObservationBackend::new(SharedRegisterObserver::new(recorder.clone()));
    backend.reserve(2)?.emit(vec![
        RegisterObservation::write(0, AccessOrigin::Direct, 8, &[0x0b, 0xa1]),
        RegisterObservation::read(1, AccessOrigin::Direct, 8, &[0x07, 0xb2]),
    ])?;
    let transcript = lock(&recorder.0).clone();
    Ok(transcript)
}

fn blocking_observations() -> Result<Vec<RegisterObservation>, Box<dyn std::error::Error>> {
    let recorder = Arc::new(Recorder::default());
    let backend = BlockingObservationBackend::new(SharedRegisterObserver::new(recorder.clone()));
    backend.reserve(2)?.emit(vec![
        RegisterObservation::write(0, AccessOrigin::Direct, 8, &[0x0b, 0xa1]),
        RegisterObservation::read(1, AccessOrigin::Direct, 8, &[0x07, 0xb2]),
    ])?;
    let transcript = lock(&recorder.0).clone();
    Ok(transcript)
}

#[test]
fn typed_image_nonzero_bus_and_observation_policies_share_literal_results()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one authoritative image at a nonzero full-width base.
    let image = RegisterImage::<ConsumerRegisters, LocalStorage<Vec<u8>>>::new()?;
    let base = RegisterBusBase::new(0x1_0000_4000);
    let bus = RegisterBus::new(base, image.clone());

    // When: typed access writes, then addressed raw access reads and writes the same leaf.
    image.view_mut(|mut root| -> Result<(), Box<dyn std::error::Error>> {
        Ok(root.queues_mut()?.control_mut()?.set_mode(5)?)
    })??;
    let mut bytes = [0_u8; 2];
    bus.read(RegisterBusAddress::new(base.raw() + 8), &mut bytes)?;
    bus.write(RegisterBusAddress::new(base.raw() + 8), &[0x07, 0xb2])?;
    let typed = image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
        Ok(root.queues()?.control()?.value()?)
    })??;
    let local = local_observations()?;
    let blocking = blocking_observations()?;

    // Then: typed/raw values and local/blocking observation transcripts are exact.
    assert_eq!(bytes, [0x0b, 0xa1]);
    assert_eq!(typed, 0xb207);
    assert_eq!(local, blocking);
    assert_eq!(local.len(), 2);
    assert_eq!(
        local.first().map(|item| item.request_bytes.as_slice()),
        Some([0x0b, 0xa1].as_slice())
    );
    assert_eq!(
        local.get(1).map(|item| item.response_bytes.as_slice()),
        Some([0x07, 0xb2].as_slice())
    );
    println!(
        "register base=0x100004000 typed=0xb207 raw_before=[0b,a1] observation_local=2 observation_blocking=2 transcript_equal=true"
    );
    Ok(())
}
