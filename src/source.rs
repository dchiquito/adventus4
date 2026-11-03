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
pub fn tmp_test_file_path(source: &str) -> PathBuf {
    let hash = {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::hash::DefaultHasher::new();
        source.hash(&mut hasher);
        hasher.finish()
    };
    let unique_name = format!("{hash}.adv");
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
