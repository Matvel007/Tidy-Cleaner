use std::collections::HashSet;

const EN_XML: &str = include_str!("../resources/i18n/en.xml");
const RU_XML: &str = include_str!("../resources/i18n/ru.xml");

fn parse_keys(xml: &str) -> (HashSet<String>, Vec<(String, String)>) {
    let mut reader = quick_xml::reader::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut keys = HashSet::new();
    let mut pairs = Vec::new();
    let mut current_key: Option<String> = None;

    loop {
        match reader.read_event().expect("XML parsing error") {
            quick_xml::events::Event::Start(e) if e.name().as_ref() == b"string" => {
                for attr in e.attributes() {
                    let attr = attr.expect("Attribute error");
                    if attr.key.as_ref() == b"name" {
                        let key = String::from_utf8(attr.value.into_owned()).expect("UTF-8 error");
                        assert!(!keys.contains(&key), "Duplicate key '{}' found in XML", key);
                        keys.insert(key.clone());
                        current_key = Some(key);
                    }
                }
            }
            quick_xml::events::Event::Text(e) => {
                if let Some(ref key) = current_key {
                    let val = e.unescape().expect("Unescape error").into_owned();
                    pairs.push((key.clone(), val));
                }
            }
            quick_xml::events::Event::End(e) if e.name().as_ref() == b"string" => {
                current_key = None;
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }

    (keys, pairs)
}

#[test]
fn test_all_keys_match_between_en_and_ru() {
    let (en_keys, en_pairs) = parse_keys(EN_XML);
    let (ru_keys, ru_pairs) = parse_keys(RU_XML);

    assert!(!en_keys.is_empty(), "en.xml should not be empty");
    assert!(!ru_keys.is_empty(), "ru.xml should not be empty");

    let missing_in_ru: Vec<_> = en_keys.difference(&ru_keys).collect();
    let missing_in_en: Vec<_> = ru_keys.difference(&en_keys).collect();

    assert!(
        missing_in_ru.is_empty(),
        "Keys present in en.xml but missing in ru.xml: {:?}",
        missing_in_ru
    );

    assert!(
        missing_in_en.is_empty(),
        "Keys present in ru.xml but missing in en.xml: {:?}",
        missing_in_en
    );

    for (k, v) in en_pairs {
        assert!(
            !v.trim().is_empty(),
            "Empty translation for key '{}' in en.xml",
            k
        );
    }

    for (k, v) in ru_pairs {
        assert!(
            !v.trim().is_empty(),
            "Empty translation for key '{}' in ru.xml",
            k
        );
    }
}
