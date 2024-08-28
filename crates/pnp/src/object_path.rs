use std::fmt::{Display, Formatter};
use std::str::FromStr;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use thiserror::Error;
use crate::pcb::PcbKind;

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq)]
struct ObjectPathChunk {
    key: String,
    value: String,
}

impl FromStr for ObjectPathChunk {
    type Err = ObjectPathError;

    fn from_str(chunk: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = chunk.split('=').collect();
        if parts.len() == 2 {
            let (key, value) = (parts[0], parts[1]);
            
            match key {
                "single" |
                "panel" |
                "unit" => {
                    let index: usize = value.parse().map_err(|_err|
                        ObjectPathError::InvalidIndex(value.to_string())
                    )?;
                    Ok(ObjectPathChunk { key: key.to_string(), value: index.to_string() })
                },
                "ref_des" => {
                    Ok(ObjectPathChunk { key: key.to_string(), value: value.to_string() })
                }
                _ => Err(ObjectPathError::UnknownKey(key.to_string()))
            }
        } else {
            Err(ObjectPathError::InvalidChunk(chunk.to_string()))
        }
    }
}

impl Display for ObjectPathChunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// A path to an object
///
/// `<("panel"|"single")=<index>::"unit"=<index>[::("ref_dex")=<ref_des>]`
///
/// e.g.
///
/// `panel=1::unit=1`
/// `panel=1::unit=1::ref_des=R1` 
#[derive(Debug, Clone, DeserializeFromStr, SerializeDisplay, PartialOrd, Ord, Eq, PartialEq, Default)]
pub struct ObjectPath {
    chunks: Vec<ObjectPathChunk>,
}

impl ObjectPath {
    pub fn set_ref_des(&mut self, ref_des: String) {
        self.set_chunk(ObjectPathChunk { key: "ref_des".to_string(), value: ref_des })
    }

    pub fn pcb_unit(&self) -> ObjectPath {
        // TODO consider replacing 'panel' and 'single' with just 'pcb', since the pcb defines the kind now.
        const PCB_UNIT_KEYS: [&str; 3] = ["panel", "single", "unit"];
        
        self.chunks.iter().fold(ObjectPath::default(), | mut object_path, chunk | {

            if PCB_UNIT_KEYS.contains(&chunk.key.as_str()) {
                object_path.chunks.push(chunk.clone())
            }
            
            object_path  
        })
    }
    
    pub fn pcb_kind_and_index(&self) -> Option<(PcbKind, usize)> {
        self.find_chunk_by_key("panel")
            .or_else(|| self.find_chunk_by_key("single"))
            .map(|chunk|(PcbKind::try_from(&chunk.key).unwrap(), chunk.value.parse().unwrap()))
    }

    fn set_chunk(&mut self, chunk: ObjectPathChunk) {
        let existing_chunk = self.find_chunk_by_key_mut(&chunk.key);
        match existing_chunk {
            Some(existing_chunk) => existing_chunk.value = chunk.value,
            _ => self.chunks.push(chunk)
        }
    }

    fn find_chunk_by_key(&self, key: &str) -> Option<&ObjectPathChunk> {
        let existing_chunk = self.chunks.iter().find(|existing| {
            existing.key.eq(key)
        });
        
        existing_chunk
    }

    fn find_chunk_by_key_mut(&mut self, key: &str) -> Option<&mut ObjectPathChunk> {
        let existing_chunk = self.chunks.iter_mut().find(|existing| {
            existing.key.eq(key)
        });

        existing_chunk
    }
}

#[cfg(test)]
mod pcb_unit_tests {
    use rstest::rstest;
    use super::*;
    
    #[rstest]
    #[case("panel=1")]
    #[case("single=1")]
    #[case("panel=1::unit=1")]
    #[case("single=1::unit=1")]
    #[case("panel=1::unit=1::ref_des=R1")]
    #[case("single=1::unit=1::ref_des=R1")]
    pub fn from_str(#[case] input: &str) {
        // expect
        ObjectPath::from_str(input).expect("ok");
    }

    #[rstest]
    #[case("bad", Err(ObjectPathError::InvalidChunk("bad".to_string())))]
    #[case("foo=bar", Err(ObjectPathError::UnknownKey("foo".to_string())))]
    #[case("panel=1::", Err(ObjectPathError::InvalidChunk("".to_string())))]
    /// the invalid trailing ':' becomes part of the index.
    #[case("panel=1:", Err(ObjectPathError::InvalidIndex("1:".to_string())))]
    #[case("panel=1::::ref_des=R1", Err(ObjectPathError::InvalidChunk("".to_string())))]
    pub fn from_str_errors(#[case] input: &str, #[case] expected_result: Result<ObjectPath, ObjectPathError>) {

        // expect
        assert_eq!(ObjectPath::from_str(input), expected_result);
    }
    
    #[test]
    pub fn pcb_unit() {
        // given
        let object_path = ObjectPath::from_str("panel=1::unit=1::ref_des=R1").expect("always ok");

        // and
        let expected_result = ObjectPath::from_str("panel=1::unit=1").expect("always ok");

        // when
        let result = object_path.pcb_unit();

        // then
        assert_eq!(result, expected_result);
    }
    
    #[test]
    pub fn set_ref_des() {
        // given
        let mut object_path = ObjectPath::from_str("panel=1::unit=1").expect("always ok");
        
        // and
        let expected_result= ObjectPath::from_str("panel=1::unit=1::ref_des=R1").expect("always ok");
        
        // when
        object_path.set_ref_des("R1".to_string());
        
        // then
        assert_eq!(object_path, expected_result);
    }
    
    #[test]
    pub fn update_ref_des() {
        // given
        let mut object_path = ObjectPath::from_str("panel=1::unit=1::ref_des=R2").expect("always ok");
        
        // and
        let expected_result= ObjectPath::from_str("panel=1::unit=1::ref_des=R1").expect("always ok");
        
        // when
        object_path.set_ref_des("R1".to_string());
        
        // then
        assert_eq!(object_path, expected_result);
    }
}

impl Display for ObjectPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted_chunks: Vec<String> = self.chunks.iter()
            .map(|chunk| format!("{}", chunk))
            .collect();
        
        write!(f, "{}",
           formatted_chunks.join("::")
        )
    }
}

impl FromStr for ObjectPath {
    type Err = ObjectPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.split("::")
            .try_fold(ObjectPath::default(), |mut object_path: ObjectPath, chunk_str| {
                match ObjectPathChunk::from_str(chunk_str) {
                    Ok(chunk) => {
                        object_path.chunks.push(chunk);
                        Ok(object_path)
                    },
                    Err(err) => Err(err),
                }
            })
        
        // TODO validate the the order of the chunks is correct
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum ObjectPathError {
    #[error("Invalid object path. value: '{0:}'")]
    Invalid(String),
    #[error("Invalid index in path. index: '{0:}'")]
    InvalidIndex(String),
    #[error("Invalid chunk in path. chunk: '{0:}'")]
    InvalidChunk(String),
    #[error("Invalid chunk key in path. key: '{0:}'")]
    UnknownKey(String),
}
