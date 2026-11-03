use std::path::PathBuf;

const RANGE_ADV: &str = include_str!("../lib/range.adv");
const LIST_ADV: &str = include_str!("../lib/list.adv");
pub const LIBS: [&str; 2] = ["lib/range.adv", "lib/list.adv"];

pub fn read_source_file(source_name: &str) -> String {
    let path = PathBuf::from(source_name);
    if path.starts_with("lib") {
        match source_name {
            "lib/list.adv" => LIST_ADV.to_string(),
            "lib/range.adv" => RANGE_ADV.to_string(),
            _ => panic!("no source lib {source_name}"),
        }
    } else {
        std::fs::read_to_string(&path).unwrap()
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
