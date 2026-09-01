use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    #[default]
    En,
    Ru,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Ru => "ru",
        }
    }

    pub fn from_str_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "ru" | "russian" | "русский" => Language::Ru,
            _ => Language::En,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocalizationBundle {
    entries: HashMap<String, String>,
}

#[allow(dead_code)]
impl LocalizationBundle {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn from_xml(xml_content: &str) -> Result<Self> {
        let mut reader = Reader::from_str(xml_content);
        reader.config_mut().trim_text(true);

        let mut entries = HashMap::new();
        let mut current_key: Option<String> = None;

        loop {
            match reader.read_event()? {
                Event::Start(e) if e.name().as_ref() == b"string" => {
                    for attr in e.attributes() {
                        let attr = attr.context("Failed to parse XML attribute")?;
                        if attr.key.as_ref() == b"name" {
                            let key = String::from_utf8(attr.value.into_owned())
                                .context("Invalid UTF-8 in string name")?;
                            current_key = Some(key);
                        }
                    }
                }
                Event::Text(e) => {
                    if let Some(ref key) = current_key {
                        let text = e.unescape()?.into_owned();
                        entries.insert(key.clone(), text);
                    }
                }
                Event::End(e) if e.name().as_ref() == b"string" => {
                    current_key = None;
                }
                Event::Eof => break,
                _ => {}
            }
        }

        Ok(Self { entries })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    pub fn get_or_default<'a>(&'a self, key: &'a str) -> &'a str {
        self.entries.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
