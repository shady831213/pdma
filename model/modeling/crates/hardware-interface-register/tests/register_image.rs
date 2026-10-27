//! Register-image default, backend-parity, and unwind coverage.

use std::panic::{AssertUnwindSafe, catch_unwind};

use hardware_interface_register::RegisterImage;
use hardware_interface_schema::{RegisterValueError, register_map, register_schema};
use hardware_interface_storage::{LocalStorage, MutexStorage};

register_schema! {
    pub register ImageControl {
        width: 2,
        alignment: 2,
        endian: Little,
        access: ReadWrite,
        reset: Constant(0xa103),
        writable_mask: 0xff0f,
        fields: { enable: 0..1, mode: 1..4, tag: 8..16 },
    }
}

register_map! {
    pub map ImageMap {
        alignment: 4,
        extent: 8,
        placements: [control: ImageControl { offset: 4 }],
        access: [group.control => control],
    }
}

fn image_behavior<B>(image: &RegisterImage<ImageMap, B>) -> Result<u64, Box<dyn std::error::Error>>
where
    B: hardware_interface_storage::StorageBackend<Vec<u8>>,
{
    let reset = image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
        Ok(root.group()?.control()?.value()?)
    })??;
    assert_eq!(reset, 0xa103);
    image.view_mut(|mut root| -> Result<(), Box<dyn std::error::Error>> {
        Ok(root.group_mut()?.control_mut()?.set_mode(5)?)
    })??;
    let changed = image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
        Ok(root.control()?.value()?)
    })??;
    assert_eq!(changed, 0xa10b);
    Ok(changed)
}

#[test]
fn default_image_and_typed_views_are_backend_identical() -> Result<(), Box<dyn std::error::Error>> {
    let local = RegisterImage::<ImageMap, LocalStorage<Vec<u8>>>::new()?;
    let shared = RegisterImage::<ImageMap, MutexStorage<Vec<u8>>>::new()?;

    let local_value = image_behavior(&local)?;
    let shared_value = image_behavior(&shared)?;

    assert_eq!(local_value, 0xa10b);
    assert_eq!(shared_value, local_value);
    Ok(())
}

#[test]
fn adverse_field_error_and_panic_commit_nothing_and_storage_recovers()
-> Result<(), Box<dyn std::error::Error>> {
    let image = RegisterImage::<ImageMap, LocalStorage<Vec<u8>>>::new()?;
    let before = image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
        Ok(root.control()?.value()?)
    })??;

    let error = image.view_mut(|mut root| -> Result<(), Box<dyn std::error::Error>> {
        Ok(root.group_mut()?.control_mut()?.set_mode(8)?)
    })?;
    assert!(matches!(
        error.as_ref().err().and_then(|error| error.downcast_ref()),
        Some(RegisterValueError::FieldValueOverflow { location, bit_width: 3, value: 8 })
            if *location == ImageControl::MODE_IDENTITY.at_bit(33)
    ));
    assert_eq!(
        image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
            Ok(root.control()?.value()?)
        })??,
        before
    );

    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = image.view_mut::<()>(|mut root| {
            if let Ok(mut control) = root.control_mut() {
                let _ = control.set_mode(6);
            }
            panic!("inspection panic")
        });
    }));
    assert!(panic.is_err());
    assert_eq!(
        image.view(|root| -> Result<u64, Box<dyn std::error::Error>> {
            Ok(root.control()?.value()?)
        })??,
        0xa103
    );
    Ok(())
}
