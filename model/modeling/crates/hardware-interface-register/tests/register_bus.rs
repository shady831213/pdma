//! Addressed register-bus authority, validation, and backend coverage.

use hardware_interface_register::{
    RegisterBus, RegisterBusAddress, RegisterBusBase, RegisterBusError, RegisterEndpointError,
    RegisterImage,
};
use hardware_interface_schema::{RegisterValueError, register_map, register_schema};
use hardware_interface_storage::{LocalStorage, MutexStorage};

register_schema! {
    pub register BusControl {
        width: 2, alignment: 2, endian: Little, access: ReadWrite,
        reset: Constant(0xa103), writable_mask: 0xff0f,
        fields: { enable: 0..1, mode: 1..4, tag: 8..16 },
    }
}
register_schema! {
    pub register BusStatus {
        width: 4, alignment: 4, endian: Little, access: ReadOnly,
        reset: PostReset(0x4433_2211), writable_mask: 0, fields: {},
    }
}
register_schema! {
    pub register BusDoorbell {
        width: 1, alignment: 1, endian: Little, access: WriteOnly,
        reset: Zero(0), writable_mask: 0xff, fields: {},
    }
}
register_map! {
    pub map BusMap {
        alignment: 8,
        extent: 24,
        placements: [
            status: BusStatus,
            control: BusControl { offset: 8 },
            doorbell: BusDoorbell { offset: 16 },
        ],
        access: [group.status => status, group.control => control, group.doorbell => doorbell],
    }
}

const BASE: RegisterBusBase = RegisterBusBase::new(0x1_0000_1000);

fn authoritative<B>(image: &RegisterImage<BusMap, B>) -> Result<(), Box<dyn std::error::Error>>
where
    B: hardware_interface_storage::StorageBackend<Vec<u8>> + Clone,
{
    let bus = RegisterBus::new(BASE, image.clone());
    image.view_mut(|mut root| -> Result<(), Box<dyn std::error::Error>> {
        Ok(root.group_mut()?.control_mut()?.set_mode(5)?)
    })??;
    let mut bytes = [0_u8; 2];
    bus.read(RegisterBusAddress::new(BASE.raw() + 8), &mut bytes)?;
    assert_eq!(bytes, [0x0b, 0xa1]);

    bus.write(RegisterBusAddress::new(BASE.raw() + 8), &[0x07, 0xb2])?;
    let typed = image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
        Ok(root.group()?.control()?.value()?)
    })??;
    assert_eq!(typed, 0xb207);
    println!("authoritative_image base=0x100001000 offset=8 typed=b207 raw=[07,b2]");
    Ok(())
}

#[test]
fn authoritative_image_local() -> Result<(), Box<dyn std::error::Error>> {
    authoritative(&RegisterImage::<BusMap, LocalStorage<Vec<u8>>>::new()?)
}

#[test]
fn authoritative_image_mutex() -> Result<(), Box<dyn std::error::Error>> {
    authoritative(&RegisterImage::<BusMap, MutexStorage<Vec<u8>>>::new()?)
}

fn endpoint(
    result: Result<(), RegisterBusError<RegisterEndpointError>>,
) -> Result<RegisterEndpointError, Box<dyn std::error::Error>> {
    match result {
        Err(RegisterBusError::Endpoint { source, .. }) => Ok(source),
        Err(other) => {
            Err(std::io::Error::other(format!("expected endpoint error, got {other:?}")).into())
        }
        Ok(()) => Err(std::io::Error::other("expected endpoint error, got success").into()),
    }
}

#[test]
fn adverse_address_width_access_and_mask_errors_are_non_mutating()
-> Result<(), Box<dyn std::error::Error>> {
    adverse_address(&RegisterImage::<BusMap, LocalStorage<Vec<u8>>>::new()?)?;
    adverse_address(&RegisterImage::<BusMap, MutexStorage<Vec<u8>>>::new()?)?;
    println!(
        "adverse parity=local+mutex below overflow interior reserved width access mask unchanged=true"
    );
    Ok(())
}

fn adverse_address<B>(image: &RegisterImage<BusMap, B>) -> Result<(), Box<dyn std::error::Error>>
where
    B: hardware_interface_storage::StorageBackend<Vec<u8>> + Clone,
{
    let bus = RegisterBus::new(BASE, image.clone());
    let before = image.view(|root| -> Result<_, Box<dyn std::error::Error>> {
        Ok((
            root.status()?.value()?,
            root.control()?.value()?,
            root.doorbell()?.value()?,
        ))
    })??;
    let mut destination = [0x5a_u8; 4];

    let below = bus.read(RegisterBusAddress::new(BASE.raw() - 1), &mut destination);
    assert!(
        matches!(below, Err(RegisterBusError::AddressBelowBase { base, address, width })
        if base == BASE && address == RegisterBusAddress::new(BASE.raw() - 1) && width.raw() == 4)
    );
    let overflow = RegisterBus::new(RegisterBusBase::new(0), image.clone()).read(
        RegisterBusAddress::new(u64::from(u32::MAX) + 1),
        &mut destination,
    );
    assert!(
        matches!(overflow, Err(RegisterBusError::OffsetOutOfRange { base, address, width, delta })
        if base == RegisterBusBase::new(0) && address == RegisterBusAddress::new(u64::from(u32::MAX) + 1) && width.raw() == 4 && delta == u64::from(u32::MAX) + 1)
    );
    assert!(matches!(
        endpoint(bus.read(RegisterBusAddress::new(BASE.raw() + 1), &mut destination))?,
        RegisterEndpointError::UnmappedOffset { .. }
    ));
    assert!(matches!(
        endpoint(bus.read(RegisterBusAddress::new(BASE.raw() + 4), &mut destination))?,
        RegisterEndpointError::UnmappedOffset { .. }
    ));
    assert!(matches!(
        endpoint(bus.read(RegisterBusAddress::new(BASE.raw() + 8), &mut destination))?,
        RegisterEndpointError::WidthMismatch {
            expected: 2,
            actual: 4,
            ..
        }
    ));
    assert!(matches!(
        endpoint(bus.write(RegisterBusAddress::new(BASE.raw()), &[0; 4]))?,
        RegisterEndpointError::WriteForbidden { .. }
    ));
    assert!(matches!(
        endpoint(bus.read(RegisterBusAddress::new(BASE.raw() + 16), &mut [0]))?,
        RegisterEndpointError::ReadForbidden { .. }
    ));
    assert!(matches!(
        endpoint(bus.write(RegisterBusAddress::new(BASE.raw() + 8), &[0x13, 0xa1]))?,
        RegisterEndpointError::ReservedBitChange { changed: 0x10, .. }
    ));

    assert_eq!(destination, [0x5a; 4]);
    let after = image.view(|root| -> Result<_, Box<dyn std::error::Error>> {
        Ok((
            root.status()?.value()?,
            root.control()?.value()?,
            root.doorbell()?.value()?,
        ))
    })??;
    assert_eq!(after, before);
    Ok(())
}

#[test]
fn adverse_typed_field_overflow_and_backend_reentry_preserve_precedence()
-> Result<(), Box<dyn std::error::Error>> {
    let image = RegisterImage::<BusMap, LocalStorage<Vec<u8>>>::new()?;
    let bus = RegisterBus::new(BASE, image.clone());
    let before = image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
        Ok(root.control()?.value()?)
    })??;
    let field = image.view_mut(|mut root| -> Result<(), Box<dyn std::error::Error>> {
        Ok(root.group_mut()?.control_mut()?.set_mode(8)?)
    })?;
    assert!(matches!(
        field.as_ref().err().and_then(|error| error.downcast_ref()),
        Some(RegisterValueError::FieldValueOverflow { location, bit_width: 3, value: 8 })
            if *location == BusControl::MODE_IDENTITY.at_bit(65)
    ));

    let nested =
        image.view(|_| bus.write(RegisterBusAddress::new(BASE.raw() + 8), &[0x07, 0xa1]))?;
    assert!(matches!(
        nested,
        Err(RegisterBusError::Endpoint {
            source: RegisterEndpointError::Storage(_),
            ..
        })
    ));
    assert_eq!(
        image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
            Ok(root.control()?.value()?)
        })??,
        before
    );
    println!("adverse field_overflow=8 reentry=storage unchanged=true");
    Ok(())
}
