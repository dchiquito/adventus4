use std::{collections::HashMap, ops::Range, path::PathBuf};

use crate::bytecode::{SourceId, SourceMap, Span};

const BOOL_ADV: &str = include_str!("../lib/bool.adv");
const IO_ADV: &str = include_str!("../lib/io.adv");
const RANGE_ADV: &str = include_str!("../lib/range.adv");
const LIST_ADV: &str = include_str!("../lib/list.adv");
pub const LIBS: [&str; 4] = [
    "lib/bool.adv",
    "lib/io.adv",
    "lib/range.adv",
    "lib/list.adv",
];

pub fn read_source_file(source_name: &str) -> String {
    let path = PathBuf::from(source_name);
    if path.starts_with("lib") {
        match source_name {
            "lib/bool.adv" => BOOL_ADV.to_string(),
            "lib/io.adv" => IO_ADV.to_string(),
            "lib/list.adv" => LIST_ADV.to_string(),
            "lib/range.adv" => RANGE_ADV.to_string(),
            _ => panic!("no source lib {source_name}"),
        }
    } else {
        std::fs::read_to_string(&path).unwrap()
    }
}

pub struct SourceViewer {
    source_names: HashMap<SourceId, String>,
    sources: HashMap<SourceId, String>,
}
impl SourceViewer {
    pub fn new(source_map: &SourceMap) -> Self {
        let source_names = source_map
            .iter_source_names()
            .map(|(id, name)| (id, name.clone()))
            .collect();
        let sources = source_map
            .iter_source_names()
            .map(|(id, name)| (id, read_source_file(name)))
            .collect();
        Self {
            source_names,
            sources,
        }
    }
    pub fn get_source_name(&self, span: &Span) -> &str {
        self.source_names
            .get(&span.source_id)
            .unwrap_or_else(|| panic!("unknown {:?}", span.source_id))
    }
    pub fn get_source(&self, span: &Span) -> &str {
        self.sources
            .get(&span.source_id)
            .unwrap_or_else(|| panic!("unknown {:?}", span.source_id))
    }
    pub fn get_str(&self, span: &Span) -> &str {
        &self.get_source(span)[span.range.clone()]
    }
    pub fn get_source_line_extents(&self, span: &Span) -> Range<usize> {
        let range = span.range.clone();
        let source = self.get_source(span);
        let start_offset = source.as_bytes()[..range.start]
            .iter()
            .rev()
            .position(|&c| c == b'\n')
            .unwrap_or(range.start);
        let end_offset = source.as_bytes()[range.end..]
            .iter()
            .position(|&c| c == b'\n')
            .unwrap_or(source.len() - range.end);
        (range.start - start_offset)..(range.end + end_offset)
    }
    pub fn get_line_number(&self, span: &Span) -> usize {
        let source = self.get_source(span);
        let mut lines = 1;
        for i in 0..span.range.start {
            if source.as_bytes()[i] == b'\n' {
                lines += 1;
            }
        }
        lines
    }
}

#[cfg(test)]
const BASE64URL: [char; 64] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '-', '_',
];

#[cfg(test)]
fn base64_encode_hash(hash: u64) -> String {
    let mut hash = hash;
    let mut buf = String::new();
    for _ in 0..11 {
        let octet = (hash & 63) as usize;
        buf.push(BASE64URL[octet]);
        hash >>= 6;
    }
    buf.push_str(".adv");
    buf
}

#[cfg(test)]
pub fn tmp_test_file_path(source: &str) -> PathBuf {
    let hash = {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::hash::DefaultHasher::new();
        source.hash(&mut hasher);
        hasher.finish()
    };
    let unique_name = base64_encode_hash(hash);
    {
        let mut tmp_source = PathBuf::from("/tmp");
        tmp_source.push(unique_name);
        tmp_source
    }
}

#[cfg(test)]
pub fn write_test_file(source: &str) -> std::io::Result<PathBuf> {
    let path = tmp_test_file_path(source);
    std::fs::write(&path, source)?;
    Ok(path)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read_source_file() {
        for lib in LIBS {
            assert!(!read_source_file(lib).is_empty());
        }
    }
    #[test]
    #[should_panic]
    fn test_read_source_file_panics() {
        read_source_file("lib/does_not_exist");
    }
}
