#![allow(missing_docs, reason = "integration tests")]

use hardware_interface_io::memory::{
    AtomicAccessError, AtomicOperation, AtomicOrder, AtomicReadOrder, AtomicUpdate, MemoryAccess,
    MemoryAddress, MemoryAlignment,
};
use hardware_interface_io::memory::{
    LocalMemoryPolicy, Memory, MemoryPolicy, MemoryRegion, SharedMemoryPolicy,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn memory<P: MemoryPolicy>() -> Result<Memory<P>, Box<dyn std::error::Error>> {
    let memory = Memory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        16,
        MemoryAlignment::try_new(8)?,
    )?])?;
    Ok(memory)
}

#[test]
fn updates_return_the_previous_value_and_commit_once() -> TestResult {
    // Given: the same word and update cases in local and shared storage.
    fn check<P: MemoryPolicy>() -> TestResult {
        let memory = memory::<P>()?;
        let cases = [
            (AtomicUpdate::Add(5), 17, 22),
            (AtomicUpdate::Subtract(5), 17, 12),
            (AtomicUpdate::And(6), 11, 2),
            (AtomicUpdate::Or(6), 9, 15),
            (AtomicUpdate::Xor(6), 11, 13),
            (AtomicUpdate::Exchange(29), 17, 29),
            (
                AtomicUpdate::CompareExchange {
                    current: 17,
                    new: 31,
                },
                17,
                31,
            ),
            (
                AtomicUpdate::CompareExchange {
                    current: 18,
                    new: 31,
                },
                17,
                17,
            ),
            (AtomicUpdate::Add(1), u64::MAX, 0),
            (AtomicUpdate::Subtract(1), 0, u64::MAX),
        ];
        for (update, initial, expected) in cases {
            memory.write(0x1000.into(), MemoryAlignment::ONE, &initial.to_le_bytes())?;
            // When: one indivisible update is issued.
            let old = memory.atomic64(
                0x1000.into(),
                AtomicOperation::Update {
                    update,
                    order: AtomicOrder::AcquireRelease,
                },
            )?;
            // Then: it returns the old word and leaves exactly the expected new word.
            assert_eq!(old, initial);
            assert_eq!(
                memory.atomic64(
                    0x1000.into(),
                    AtomicOperation::Load(AtomicReadOrder::Acquire)
                )?,
                expected
            );
        }
        Ok(())
    }
    check::<LocalMemoryPolicy>()?;
    check::<SharedMemoryPolicy>()
}

#[test]
fn narrow_updates_do_not_modify_the_adjacent_word() -> TestResult {
    // Given: two adjacent words with distinct bytes.
    let memory = memory::<LocalMemoryPolicy>()?;
    let initial = u32::MAX;
    let adjacent = 0x1234_5678_u32;
    memory.write(0x1000.into(), MemoryAlignment::ONE, &initial.to_le_bytes())?;
    memory.write(0x1004.into(), MemoryAlignment::ONE, &adjacent.to_le_bytes())?;
    // When: a 32-bit addition wraps.
    let old = memory.atomic32(
        0x1000.into(),
        AtomicOperation::Update {
            update: AtomicUpdate::Add(1),
            order: AtomicOrder::Relaxed,
        },
    )?;
    // Then: only that word changes, with a width-correct old value.
    assert_eq!(old, initial);
    assert_eq!(
        memory.atomic32(
            0x1000.into(),
            AtomicOperation::Load(AtomicReadOrder::Relaxed)
        )?,
        0
    );
    assert_eq!(
        memory.atomic32(
            0x1004.into(),
            AtomicOperation::Load(AtomicReadOrder::Relaxed)
        )?,
        adjacent
    );
    Ok(())
}

#[test]
fn word_updates_apply_every_operation_at_the_selected_width() -> TestResult {
    // Given: independent 32-bit update cases, including wrapping and both CAS outcomes.
    let memory = memory::<SharedMemoryPolicy>()?;
    let cases = [
        (AtomicUpdate::Add(1), u32::MAX, 0),
        (AtomicUpdate::Subtract(1), 0, u32::MAX),
        (AtomicUpdate::And(6), 11, 2),
        (AtomicUpdate::Or(6), 9, 15),
        (AtomicUpdate::Xor(6), 11, 13),
        (AtomicUpdate::Exchange(29), 17, 29),
        (
            AtomicUpdate::CompareExchange {
                current: 17,
                new: 31,
            },
            17,
            31,
        ),
        (
            AtomicUpdate::CompareExchange {
                current: 18,
                new: 31,
            },
            17,
            17,
        ),
    ];
    for order in [
        AtomicOrder::Relaxed,
        AtomicOrder::Acquire,
        AtomicOrder::Release,
        AtomicOrder::AcquireRelease,
    ] {
        for (update, initial, expected) in cases {
            memory.write(0x1000.into(), MemoryAlignment::ONE, &initial.to_le_bytes())?;
            // When: the operation is performed with each legal RMW ordering.
            let old = memory.atomic32(0x1000.into(), AtomicOperation::Update { update, order })?;
            // Then: return and final bits agree with the independent arithmetic cases.
            assert_eq!(old, initial);
            assert_eq!(
                memory.atomic32(
                    0x1000.into(),
                    AtomicOperation::Load(AtomicReadOrder::Acquire)
                )?,
                expected
            );
        }
    }
    Ok(())
}

#[test]
fn rejected_atomic_access_preserves_storage() -> TestResult {
    // Given: a mapped region surrounded by unmapped addresses.
    let memory = memory::<SharedMemoryPolicy>()?;
    let before = [0x7a; 16];
    memory.write(0x1000.into(), MemoryAlignment::ONE, &before)?;
    // When: invalid atomic requests reach the backend.
    for address in [0x1001, 0x1010] {
        assert!(matches!(
            memory.atomic64(
                address.into(),
                AtomicOperation::Update {
                    update: AtomicUpdate::Add(1),
                    order: AtomicOrder::Relaxed,
                }
            ),
            Err(AtomicAccessError::Access(_))
        ));
    }
    // Then: neither request changes any mapped byte.
    let mut after = [0; 16];
    memory.read(0x1000.into(), MemoryAlignment::ONE, &mut after)?;
    assert_eq!(after, before);
    Ok(())
}

#[test]
fn concurrent_updates_form_one_sequence_of_old_values() -> TestResult {
    // Given: independently cloned accessors to one shared counter.
    let memory = memory::<SharedMemoryPolicy>()?;
    let workers = 4;
    let increments = 1000;
    // When: all workers increment the same word concurrently.
    let mut old_values = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..workers)
            .map(|_| {
                let memory = memory.clone();
                scope.spawn(move || {
                    (0..increments)
                        .map(|_| {
                            memory.atomic64(
                                0x1000.into(),
                                AtomicOperation::Update {
                                    update: AtomicUpdate::Add(1),
                                    order: AtomicOrder::Relaxed,
                                },
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
            })
            .collect();
        let mut values = Vec::new();
        for worker in threads {
            values.extend(worker.join().map_err(|_| "atomic worker panicked")??);
        }
        Ok::<_, Box<dyn std::error::Error>>(values)
    })?;
    // Then: no increments are lost and each old value was claimed exactly once.
    old_values.sort_unstable();
    assert_eq!(old_values, (0..workers * increments).collect::<Vec<_>>());
    assert_eq!(
        memory.atomic64(
            0x1000.into(),
            AtomicOperation::Load(AtomicReadOrder::Acquire)
        )?,
        workers * increments
    );
    Ok(())
}

#[test]
fn ordinary_word_access_preserves_adjacent_storage() -> TestResult {
    // Given: ordinary accesses through both existing storage policies.
    fn check<P: MemoryPolicy>() -> TestResult {
        let memory = memory::<P>()?;
        let expected = [0x35; 16];
        // When: one region is written and read through the byte-access capability.
        memory.write(0x1000.into(), MemoryAlignment::ONE, &expected)?;
        let mut actual = [0; 16];
        memory.read(0x1000.into(), MemoryAlignment::ONE, &mut actual)?;
        // Then: all bytes survive independently of the storage policy.
        assert_eq!(actual, expected);
        Ok(())
    }
    check::<LocalMemoryPolicy>()?;
    check::<SharedMemoryPolicy>()
}
