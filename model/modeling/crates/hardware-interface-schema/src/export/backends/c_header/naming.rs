pub(super) fn c_identifier(input: &str) -> String {
    let mut output = String::with_capacity(input.len().saturating_mul(2).saturating_add(2));
    output.push_str("C_");
    for character in input.chars() {
        match character {
            'a'..='p' | 'r'..='z' => output.push(character.to_ascii_uppercase()),
            'q' => output.push_str("QQ"),
            'A'..='Z' => {
                output.push_str("QU");
                output.push(character);
            }
            '0'..='9' => output.push(character),
            '_' => output.push_str("QS"),
            _ => push_encoded_bytes(&mut output, character),
        }
    }
    output
}

fn push_encoded_bytes(output: &mut String, character: char) {
    const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = [0_u8; 4];
    for byte in character.encode_utf8(&mut encoded).bytes() {
        output.push_str("QX");
        push_hex_digit(output, HEX_DIGITS, byte >> 4);
        push_hex_digit(output, HEX_DIGITS, byte & 0x0f);
    }
}

fn push_hex_digit(output: &mut String, digits: &[u8; 16], nibble: u8) {
    if let Some(digit) = digits.get(usize::from(nibble)) {
        output.push(char::from(*digit));
    }
}

pub(super) fn disambiguate_identifiers(identifiers: Vec<String>) -> Vec<String> {
    let mut totals = std::collections::BTreeMap::<String, usize>::new();
    for identifier in &identifiers {
        *totals.entry(identifier.clone()).or_default() += 1;
    }
    let mut occurrences = std::collections::BTreeMap::<String, usize>::new();
    identifiers
        .into_iter()
        .map(|identifier| match totals.get(&identifier) {
            Some(1) | None => identifier,
            Some(_) => {
                let occurrence = occurrences.entry(identifier.clone()).or_default();
                *occurrence += 1;
                format!("{identifier}__{occurrence}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{c_identifier, disambiguate_identifiers};

    #[test]
    fn identifier_is_stable_when_input_contains_special_characters() {
        // Given: a name that begins with a digit and contains repeated punctuation.
        let input = "9 weird--name!";

        // When: it crosses the C identifier boundary.
        let identifier = c_identifier(input);

        // Then: the result is a portable, injective identifier.
        assert_eq!(identifier, "C_9QX20WEIRDQX2DQX2DNAMEQX21");
    }

    #[test]
    fn identifier_has_a_fallback_when_input_has_no_identifier_characters() {
        // Given: a name containing punctuation only.
        let input = "---";

        // When: it crosses the C identifier boundary.
        let identifier = c_identifier(input);

        // Then: every byte has a deterministic portable representation.
        assert_eq!(identifier, "C_QX2DQX2DQX2D");
    }

    #[test]
    fn identifiers_keep_declaration_order_when_raw_names_repeat() {
        // Given: two identical names in one declaration scope.
        let inputs = vec![c_identifier("same"), c_identifier("same")];

        // When: the identifiers are disambiguated for one declaration scope.
        let identifiers = disambiguate_identifiers(inputs);

        // Then: each declaration remains readable and unambiguous.
        assert_eq!(identifiers, ["C_SAME__1", "C_SAME__2"]);
    }

    #[test]
    fn identifiers_distinguish_case_punctuation_underscore_and_unicode() {
        // Given: caller names that lossy uppercasing and punctuation folding would collapse.
        let inputs = ["a-b", "a_b", "A_B", "a b", "ab", "", "q", "你"];

        // When: every name crosses the C identifier boundary.
        let identifiers = inputs.map(c_identifier);

        // Then: every distinct input owns a distinct deterministic namespace.
        assert_eq!(identifiers[0], "C_AQX2DB");
        assert_eq!(identifiers[1], "C_AQSB");
        assert_eq!(identifiers[2], "C_QUAQSQUB");
        assert_eq!(identifiers[5], "C_");
        assert_eq!(identifiers[6], "C_QQ");
        assert_eq!(identifiers[7], "C_QXE4QXBDQXA0");
        let unique = identifiers
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), inputs.len());
    }
}
