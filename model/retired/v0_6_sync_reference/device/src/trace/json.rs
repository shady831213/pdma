use pdma_model_contract::trace::TraceEvent;

pub(super) fn export(events: &[TraceEvent]) -> String {
    let mut json = String::from("{\"traceEvents\":[");
    let mut first = true;
    for event in events {
        if first {
            first = false;
        } else {
            json.push(',');
        }
        push_event(&mut json, event);
    }
    json.push_str("]}");
    json
}

fn push_event(json: &mut String, event: &TraceEvent) {
    json.push('{');
    push_field(json, "name", event.kind().name());
    json.push(',');
    push_field(json, "cat", event.operation_name());
    json.push_str(",\"ph\":\"i\",\"s\":\"t\",\"ts\":");
    json.push_str(&event.sequence().to_string());
    json.push_str(",\"pid\":0,\"tid\":0,\"args\":{");
    push_field(json, "detail", event.detail());
    if let Some(queue_id) = event.queue_id() {
        json.push_str(",\"queue\":");
        json.push_str(&queue_id.raw().to_string());
    }
    if let Some(command_id) = event.command_id() {
        json.push_str(",\"id\":");
        json.push_str(&command_id.raw().to_string());
    }
    if let Some(operation) = event.operation() {
        json.push(',');
        push_field(json, "command", operation.name());
    }
    json.push_str("}}");
}

fn push_field(json: &mut String, key: &str, value: &str) {
    json.push('"');
    push_escaped(json, key);
    json.push_str("\":\"");
    push_escaped(json, value);
    json.push('"');
}

fn push_escaped(json: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\u{08}' => json.push_str("\\b"),
            '\u{0c}' => json.push_str("\\f"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            '\u{00}'..='\u{1f}' => push_unicode_escape(json, character),
            other => json.push(other),
        }
    }
}

fn push_unicode_escape(json: &mut String, character: char) {
    json.push_str("\\u00");
    let value = u32::from(character);
    push_hex_nibble(json, value >> 4);
    push_hex_nibble(json, value);
}

fn push_hex_nibble(json: &mut String, value: u32) {
    let Some(character) = char::from_digit(value & 0x0f, 16) else {
        return;
    };
    json.push(character);
}

#[cfg(test)]
mod tests {
    use pdma_model_contract::trace::{TraceEvent, TraceEventKind};

    #[test]
    fn export_escapes_all_json_control_characters() {
        let event = TraceEvent::new(
            0,
            TraceEventKind::Idle,
            None,
            None,
            None,
            "\u{00}\u{08}\u{0c}\n\r\t\u{1f}",
        );

        let json = super::export(&[event]);

        assert!(json.contains("\\u0000\\b\\f\\n\\r\\t\\u001f"));
    }
}
