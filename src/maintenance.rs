use super::{Result, config};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::fs::DirEntry;

pub fn get_config() -> Result<config::ConfigFile> {
    use std::fs;
    let file = fs::read_to_string("./Nopain.toml")?;
    match toml::from_str(&file) {
        Ok(c) => Ok(c),
        Err(e) => Err(Box::new(e)),
    }
}

pub fn get_lock_file() -> Result<config::NopainLock> {
    use std::fs;

    if let Ok(mut file) = fs::File::open("Nopain.lock") {
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        match toml::from_str(&content) {
            Ok(c) => Ok(c),
            Err(e) => Err(Box::new(e)),
        }
    } else {
        Ok(
            config::NopainLock{
                last_build: None
            }
        )
    }
}

pub fn get_sources(path: &PathBuf, ext: &str) -> Result<Vec<DirEntry>> {
    use std::fs;
    let mut ret: Vec<DirEntry> = vec![];
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let mut inner = get_sources(&path, ext)?;
            ret.append(&mut inner);
            continue;
        }
        if let Some(ext) = path.extension() {
            if ext.eq(ext) {
                ret.push(entry);
            }
        }
    }
    Ok(ret)
}

pub fn create_lock_file(lockfile: &config::NopainLock) -> Result<()>{
    use std::fs;

    let mut f = fs::File::create("Nopain.lock")?;
    let toml = toml::to_string(lockfile)?;
    writeln!(f, "{}", toml)?;
    Ok(())
}