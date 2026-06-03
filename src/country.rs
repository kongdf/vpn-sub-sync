/// 从原节点名或域名推断国家/地区（中文简称）
pub fn detect_country(host: &str, hint: Option<&str>) -> String {
    if let Some(hint) = hint {
        if let Some(c) = match_country_text(hint) {
            return c;
        }
    }

    if let Some(c) = match_country_text(host) {
        return c;
    }

    "未知".to_string()
}

fn match_country_text(text: &str) -> Option<String> {
    let lower = text.to_lowercase();

    const RULES: &[(&[&str], &str)] = &[
        (&["香港", "hong kong", "hongkong", "🇭🇰", "hkg", "hk节点", "hk-"], "香港"),
        (&["台湾", "taiwan", "tw省", "🇹🇼", "twn", "tw-"], "台湾"),
        (&["澳门", "macau", "macao", "🇲🇴"], "澳门"),
        (&["美国", "united states", "usa", "🇺🇸", "us节点", "america"], "美国"),
        (&["日本", "japan", "tokyo", "osaka", "🇯🇵", "jp节点"], "日本"),
        (&["韩国", "korea", "seoul", "🇰🇷", "kr节点"], "韩国"),
        (&["新加坡", "singapore", "🇸🇬", "sg节点"], "新加坡"),
        (&["英国", "united kingdom", "london", "🇬🇧", "uk节点"], "英国"),
        (&["德国", "germany", "frankfurt", "🇩🇪", "de节点"], "德国"),
        (&["法国", "france", "paris", "🇫🇷", "fr节点"], "法国"),
        (&["加拿大", "canada", "toronto", "🇨🇦", "ca节点"], "加拿大"),
        (&["澳大利亚", "australia", "sydney", "🇦🇺", "au节点"], "澳大利亚"),
        (&["印度", "india", "mumbai", "🇮🇳", "in节点"], "印度"),
        (&["荷兰", "netherlands", "amsterdam", "🇳🇱", "nl节点"], "荷兰"),
        (&["俄罗斯", "russia", "moscow", "🇷🇺", "ru节点"], "俄罗斯"),
        (&["土耳其", "turkey", "istanbul", "🇹🇷", "tr节点"], "土耳其"),
        (&["阿根廷", "argentina", "🇦🇷"], "阿根廷"),
        (&["巴西", "brazil", "🇧🇷"], "巴西"),
        (&["菲律宾", "philippines", "manila", "🇵🇭"], "菲律宾"),
        (&["马来西亚", "malaysia", "kuala", "🇲🇾"], "马来西亚"),
        (&["泰国", "thailand", "bangkok", "🇹🇭"], "泰国"),
        (&["越南", "vietnam", "hanoi", "🇻🇳"], "越南"),
        (&["印尼", "indonesia", "jakarta", "🇮🇩"], "印尼"),
    ];

    for (keys, label) in RULES {
        for key in *keys {
            if lower.contains(key) {
                return Some(label.to_string());
            }
        }
    }

    // 域名常见前缀/后缀: us.example.com, hk-node.example.com
    const HOST_CODES: &[(&str, &str)] = &[
        ("-hk-", "香港"),
        (".hk.", "香港"),
        ("hk.", "香港"),
        ("-tw-", "台湾"),
        (".tw.", "台湾"),
        ("-us-", "美国"),
        (".us.", "美国"),
        ("us.", "美国"),
        ("-jp-", "日本"),
        (".jp.", "日本"),
        ("jp.", "日本"),
        ("-sg-", "新加坡"),
        (".sg.", "新加坡"),
        ("-kr-", "韩国"),
        (".kr.", "韩国"),
        ("-uk-", "英国"),
        (".uk.", "英国"),
        ("-de-", "德国"),
        (".de.", "德国"),
        ("-fr-", "法国"),
        (".fr.", "法国"),
        ("-ca-", "加拿大"),
        (".ca.", "加拿大"),
        ("-au-", "澳大利亚"),
        (".au.", "澳大利亚"),
        ("-nl-", "荷兰"),
        (".nl.", "荷兰"),
        ("-ru-", "俄罗斯"),
        (".ru.", "俄罗斯"),
    ];

    for (pattern, label) in HOST_CODES {
        if lower.contains(pattern) {
            return Some(label.to_string());
        }
    }

    // 独立国家代码 token: 前后为分隔符
    const ISO_TOKENS: &[(&str, &str)] = &[
        (" hk ", "香港"),
        (" us ", "美国"),
        (" jp ", "日本"),
        (" sg ", "新加坡"),
        (" kr ", "韩国"),
        (" tw ", "台湾"),
        (" uk ", "英国"),
        (" de ", "德国"),
        (" fr ", "法国"),
    ];
    let padded = format!(" {lower} ");
    for (token, label) in ISO_TOKENS {
        if padded.contains(token) {
            return Some(label.to_string());
        }
    }

    for token in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
        match token {
            "hk" | "hkg" => return Some("香港".to_string()),
            "tw" | "twn" => return Some("台湾".to_string()),
            "us" | "usa" => return Some("美国".to_string()),
            "jp" | "jpn" => return Some("日本".to_string()),
            "sg" | "sgp" => return Some("新加坡".to_string()),
            "kr" | "kor" => return Some("韩国".to_string()),
            "uk" | "gb" => return Some("英国".to_string()),
            "de" | "deu" => return Some("德国".to_string()),
            "fr" | "fra" => return Some("法国".to_string()),
            "ca" | "can" => return Some("加拿大".to_string()),
            "au" | "aus" => return Some("澳大利亚".to_string()),
            "nl" | "nld" => return Some("荷兰".to_string()),
            "ru" | "rus" => return Some("俄罗斯".to_string()),
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hint_chinese() {
        assert_eq!(detect_country("1.2.3.4", Some("🇭🇰香港|IEPL|01")), "香港");
    }

    #[test]
    fn from_host_pattern() {
        assert_eq!(detect_country("us-gcp.example.com", None), "美国");
    }

    #[test]
    fn unknown_fallback() {
        assert_eq!(detect_country("node.example.com", None), "未知");
    }
}
