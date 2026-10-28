use super::{RegisterMapError, RegisterMapSpec, RegisterMapValidationError};

struct Envelope {
    name: &'static str,
    start: usize,
    end: usize,
    declaration_index: usize,
}

fn envelopes<I: Copy, F: Copy>(map: RegisterMapSpec<I, F>) -> impl Iterator<Item = Envelope> {
    let fixed = map.fixed().enumerate().map(|(index, register)| Envelope {
        name: register.name(),
        start: register.offset(),
        end: register.offset().saturating_add(register.width()),
        declaration_index: register.declared_at(index),
    });
    let banks = map
        .banks()
        .iter()
        .scan(map.fixed().count(), |index, bank| {
            let declaration_index = bank
                .registers()
                .first()
                .map_or(*index, |register| register.declared_at(*index));
            *index += bank.registers().len();
            Some((declaration_index, bank))
        })
        .filter_map(|(declaration_index, bank)| {
            if bank.count() == 0 || bank.registers().is_empty() {
                return None;
            }
            let extent = bank
                .registers()
                .iter()
                .map(|register| register.offset().saturating_add(register.width()))
                .max()?;
            let end =
                crate::checked_placement_end(bank.base(), bank.count(), bank.stride(), extent)?;
            Some(Envelope {
                name: bank.name(),
                start: bank.base(),
                end,
                declaration_index,
            })
        });
    fixed.chain(banks)
}

fn overlap<I: Copy, F: Copy>(map: RegisterMapSpec<I, F>) -> Option<(Envelope, Envelope)> {
    for (index, current) in envelopes(map).enumerate() {
        for previous in envelopes(map).take(index) {
            if current.start < previous.end && previous.start < current.end {
                return Some((previous, current));
            }
        }
    }
    None
}

pub(super) fn validate<I: Copy>(
    map: RegisterMapSpec<I>,
) -> Result<(), RegisterMapValidationError<I>> {
    overlap(map).map_or(Ok(()), |(first, second)| {
        Err(RegisterMapValidationError::PlacementEnvelopeOverlap {
            first_name: first.name,
            second_name: second.name,
        })
    })
}

pub(super) fn resolve<I: Copy, F: Copy + Eq + core::fmt::Debug>(
    map: RegisterMapSpec<I, F>,
) -> Result<(), RegisterMapError<I, F>> {
    overlap(map).map_or(Ok(()), |(first, second)| {
        Err(RegisterMapError::PlacementEnvelopeOverlap {
            root: map.name(),
            path: super::resolve::join(map.name(), second.name),
            declaration_index: second.declaration_index,
            first_path: super::resolve::join(map.name(), first.name),
        })
    })
}
