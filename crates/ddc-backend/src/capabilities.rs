/// Parses VCP feature `0x60` (input select)'s enumerated allowed values out of
/// a raw DDC/CI capabilities reply string (VESA MCCS format:
/// `vcp(code1 code2 code3(v1 v2) code4 ...)`, where a code immediately
/// followed by `(...)` is an enumerated feature listing its allowed values).
/// Returns an empty `Vec` if the string has no `vcp(...)` group or feature
/// `0x60` isn't listed as enumerated.
pub fn parse_input_codes(capabilities: &str) -> Vec<u8> {
    const INPUT_SELECT: u8 = 0x60;
    let Some(vcp_start) = capabilities.find("vcp(") else {
        return Vec::new();
    };
    let rest = &capabilities[vcp_start + "vcp(".len()..];

    let mut depth: u32 = 1;
    let mut current_code: Option<u8> = None;
    let mut token = String::new();
    let mut result = Vec::new();

    let flush_value = |token: &str, current_code: Option<u8>, result: &mut Vec<u8>| {
        if current_code == Some(INPUT_SELECT) {
            if let Ok(v) = u8::from_str_radix(token.trim(), 16) {
                result.push(v);
            }
        }
    };

    for c in rest.chars() {
        match c {
            '(' => {
                depth += 1;
                current_code = u8::from_str_radix(token.trim(), 16).ok();
                token.clear();
            }
            ')' => {
                depth -= 1;
                flush_value(&token, current_code, &mut result);
                token.clear();
                if depth == 1 {
                    current_code = None;
                }
                if depth == 0 {
                    break;
                }
            }
            c if c.is_whitespace() => {
                flush_value(&token, current_code, &mut result);
                token.clear();
            }
            c => token.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic fixture in VESA MCCS capabilities-string format. Not a real
    /// captured string from the LG 34GL750 (DECISIONS.md doesn't record one) —
    /// exercises the general `code(v1 v2 ...)` grammar this parser handles.
    const FIXTURE: &str = "(prot(monitor)type(lcd)model(34GL750)cmds(01 02 03 0C E3 F3)vcp(02 04 05 08 10 12 14(05 08 0B 0C) 16 18 1A 52 60(0F 11 12) AC AE B2 B6 C6 C8 C9 D6(01 04) DF)mswhql(1)mccs_ver(2.1))";

    #[test]
    fn extracts_enumerated_values_for_feature_0x60() {
        assert_eq!(parse_input_codes(FIXTURE), vec![0x0F, 0x11, 0x12]);
    }

    #[test]
    fn returns_empty_when_feature_0x60_is_not_enumerated() {
        let no_input_select = "(prot(monitor)type(lcd)vcp(02 04 05 08 10 12))";
        assert!(parse_input_codes(no_input_select).is_empty());
    }

    #[test]
    fn returns_empty_when_there_is_no_vcp_group() {
        assert!(parse_input_codes("(prot(monitor)type(lcd))").is_empty());
    }
}
