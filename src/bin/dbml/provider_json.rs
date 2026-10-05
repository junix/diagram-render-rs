//! Allocation-bounded syntax scan before native decoding. No duplicate key may
//! disappear through serde_json::Value or an arbitrary extension map.
use anyhow::Result;
use serde::Deserializer;
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::{collections::BTreeSet, fmt};

pub(crate) const MAX_DEPTH: usize = 16;
pub(crate) const MAX_NODES: usize = 2048;

struct Seed<'a> {
    nodes: &'a mut usize,
    depth: usize,
    max_depth: usize,
    max_nodes: usize,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        if self.depth > self.max_depth || *self.nodes >= self.max_nodes {
            return Err(serde::de::Error::custom("JSON depth/node budget exceeded"));
        }
        *self.nodes += 1;
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded duplicate-free JSON")
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<(), E> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(E::custom("nonfinite number"))
        }
    }
    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq
            .next_element_seed(Seed {
                nodes: self.nodes,
                depth: self.depth + 1,
                max_depth: self.max_depth,
                max_nodes: self.max_nodes,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(serde::de::Error::custom("duplicate JSON object key"));
            }
            map.next_value_seed(Seed {
                nodes: self.nodes,
                depth: self.depth + 1,
                max_depth: self.max_depth,
                max_nodes: self.max_nodes,
            })?;
        }
        Ok(())
    }
}
pub(crate) fn validate(bytes: &[u8]) -> Result<()> {
    validate_limits(bytes, MAX_DEPTH, MAX_NODES)
}
fn validate_limits(bytes: &[u8], max_depth: usize, max_nodes: usize) -> Result<()> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    Seed {
        nodes: &mut 0,
        depth: 1,
        max_depth,
        max_nodes,
    }
    .deserialize(&mut de)?;
    de.end()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_duplicates_at_every_depth_and_escaped_spellings() {
        for bytes in [
            br#"{"a":1,"\u0061":2}"#.as_slice(),
            br#"{"x":[{"a":null,"a":false}]}"#,
            b"{} {}",
            b"1e400",
            b"NaN",
            b"\xff",
            br#""\ud800""#,
        ] {
            assert!(validate(bytes).is_err(), "{bytes:?}");
        }
        validate(br#"{"a":null,"b":[1,true,"x",{"a":2}]}"#).unwrap();
    }
    #[test]
    fn exact_depth_and_value_node_budgets_are_enforced_during_scan() {
        assert!(validate_limits(b"[[0]]", 3, 3).is_ok());
        assert!(validate_limits(b"[[0]]", 2, 3).is_err());
        assert!(validate_limits(b"[[0]]", 3, 2).is_err());
        assert!(validate_limits(b"{}", 1, 1).is_ok());
        let at_limit = format!(
            "{}0{}",
            "[".repeat(MAX_DEPTH - 1),
            "]".repeat(MAX_DEPTH - 1)
        );
        validate(at_limit.as_bytes()).unwrap();
        assert!(validate(format!("[{at_limit}]").as_bytes()).is_err());
        let nodes = format!("[{}]", vec!["0"; MAX_NODES - 1].join(","));
        validate(nodes.as_bytes()).unwrap();
        assert!(validate(format!("[0,{}]", &nodes[1..nodes.len() - 1]).as_bytes()).is_err());
    }
}
