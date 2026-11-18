//! JSON kept the way it was written: keys stay in their order, and a key may appear twice.
//!
//! Akai's screen files depend on both, so they cannot go through an ordinary JSON reader and back.

use std::fmt;

use serde::de::{Deserialize, Deserializer, Error, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::Number;
use serde_json::ser::PrettyFormatter;

const INDENT: &[u8] = b"    ";

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(Number),
    Text(String),
    List(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        serde_json::from_str(text).map_err(|error| error.to_string())
    }

    pub fn written(&self) -> String {
        let mut text = Vec::new();
        let mut writer = serde_json::Serializer::with_formatter(&mut text, PrettyFormatter::with_indent(INDENT));
        self.serialize(&mut writer).expect("writing JSON into memory cannot fail");
        String::from_utf8(text).expect("JSON is always valid text")
    }

    pub fn object<const COUNT: usize>(fields: [(&str, Json); COUNT]) -> Json {
        Json::Object(fields.into_iter().map(|(key, value)| (key.to_string(), value)).collect())
    }

    /// The first field with this key.
    pub fn field(&self, key: &str) -> Option<&Json> {
        let Json::Object(fields) = self else { return None };
        fields.iter().find(|(name, _)| name == key).map(|(_, value)| value)
    }

    pub fn field_mut(&mut self, key: &str) -> Option<&mut Json> {
        let Json::Object(fields) = self else { return None };
        fields.iter_mut().find(|(name, _)| name == key).map(|(_, value)| value)
    }

    /// Gives the field this value, in the place it has, or at the end when it is new. Only an object has fields.
    pub fn set(&mut self, key: &str, value: Json) {
        let Json::Object(fields) = self else { return };
        match fields.iter_mut().find(|(name, _)| name == key) {
            Some((_, held)) => *held = value,
            None => fields.push((key.to_string(), value)),
        }
    }

    pub fn items(&self) -> Option<&[Json]> {
        if let Json::List(items) = self { Some(items) } else { None }
    }

    pub fn items_mut(&mut self) -> Option<&mut Vec<Json>> {
        if let Json::List(items) = self { Some(items) } else { None }
    }

    pub fn as_text(&self) -> Option<&str> {
        if let Json::Text(text) = self { Some(text) } else { None }
    }
}

impl From<&str> for Json {
    fn from(text: &str) -> Self {
        Json::Text(text.to_string())
    }
}

impl From<String> for Json {
    fn from(text: String) -> Self {
        Json::Text(text)
    }
}

impl From<u32> for Json {
    fn from(number: u32) -> Self {
        Json::Number(Number::from(number))
    }
}

impl From<bool> for Json {
    fn from(truth: bool) -> Self {
        Json::Bool(truth)
    }
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Null => serializer.serialize_unit(),
            Json::Bool(truth) => serializer.serialize_bool(*truth),
            Json::Number(number) => number.serialize(serializer),
            Json::Text(text) => serializer.serialize_str(text),
            Json::List(items) => serializer.collect_seq(items),
            Json::Object(fields) => serialize_fields(fields, serializer),
        }
    }
}

fn serialize_fields<S: Serializer>(fields: &[(String, Json)], serializer: S) -> Result<S::Ok, S::Error> {
    let mut object = serializer.serialize_map(Some(fields.len()))?;
    for (key, value) in fields {
        object.serialize_entry(key, value)?;
    }
    object.end()
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(AsWritten)
    }
}

struct AsWritten;

impl<'de> Visitor<'de> for AsWritten {
    type Value = Json;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    fn visit_unit<E: Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E: Error>(self, truth: bool) -> Result<Json, E> {
        Ok(Json::Bool(truth))
    }

    fn visit_i64<E: Error>(self, number: i64) -> Result<Json, E> {
        Ok(Json::Number(Number::from(number)))
    }

    fn visit_u64<E: Error>(self, number: u64) -> Result<Json, E> {
        Ok(Json::Number(Number::from(number)))
    }

    fn visit_f64<E: Error>(self, number: f64) -> Result<Json, E> {
        Number::from_f64(number).map(Json::Number).ok_or_else(|| E::custom("a number that JSON cannot hold"))
    }

    fn visit_str<E: Error>(self, text: &str) -> Result<Json, E> {
        Ok(Json::Text(text.to_string()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut list: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = list.next_element()? {
            items.push(item);
        }
        Ok(Json::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Json, A::Error> {
        let mut fields = Vec::new();
        while let Some(field) = object.next_entry()? {
            fields.push(field);
        }
        Ok(Json::Object(fields))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of an overlay in Akai's screen files: two `version` keys in a row.
    const TWICE_VERSIONED: &str = r#"{"version": 2, "version": 1, "overlayType": "Knob", "height": 30.0, "actions": [], "nothing": null}"#;

    #[test]
    fn a_key_written_twice_is_kept_twice_and_in_order() {
        let parsed = Json::parse(TWICE_VERSIONED).unwrap();

        let Json::Object(fields) = &parsed else { panic!("expected an object") };
        let keys: Vec<&str> = fields.iter().map(|(key, _)| key.as_str()).collect();

        assert_eq!(keys, ["version", "version", "overlayType", "height", "actions", "nothing"]);
        assert_eq!(parsed.field("version"), Some(&Json::from(2)));
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let parsed = Json::parse(TWICE_VERSIONED).unwrap();

        let written = parsed.written();

        assert_eq!(Json::parse(&written).unwrap(), parsed);
        assert!(written.contains("\"height\": 30.0"), "{written}");
        assert_eq!(written.matches("\"version\"").count(), 2);
    }

    #[test]
    fn a_built_object_keeps_the_order_it_was_given() {
        let built = Json::object([("version", Json::from(2)), ("animated", Json::from(true)), ("image", Json::from("a.png"))]);

        assert_eq!(built.written(), "{\n    \"version\": 2,\n    \"animated\": true,\n    \"image\": \"a.png\"\n}");
    }

    #[test]
    fn text_that_is_not_json_is_refused() {
        assert!(Json::parse("{\"version\": ").is_err());
    }
}
