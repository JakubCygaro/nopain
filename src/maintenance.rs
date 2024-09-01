use super::{config, Result};
use std::fs::DirEntry;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::collections::HashSet;
use std::fs;

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
        Ok(config::NopainLock { last_build: None })
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

pub fn create_lock_file(lockfile: &config::NopainLock) -> Result<()> {
    use std::fs;

    let mut f = fs::File::create("Nopain.lock")?;
    let toml = toml::to_string(lockfile)?;
    writeln!(f, "{}", toml)?;
    Ok(())
}
/// Deletes all .class files in the bin/ directory of the package project
/// which were not used during compilation
pub fn purge_unused_classes(package_classes: Vec<PathBuf>) -> Result<()> {
    let package_classes = package_classes
        .into_iter()
        .collect::<HashSet<PathBuf>>();
    let bin_path = PathBuf::from("bin");
    let bin = get_sources(&bin_path, "class")?
        .into_iter()
        .map(|d| d.path())
        .map(|p| p.strip_prefix(&bin_path).unwrap().to_owned())
        .collect::<Vec<_>>();
    for class_file in &bin {
        if package_classes.contains(class_file) {
            continue;
        }
        let to_remove = bin_path.join(class_file);
        fs::remove_file(to_remove)?;
    }
    Ok(())
}
