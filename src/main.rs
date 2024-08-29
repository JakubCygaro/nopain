mod config;

use core::fmt;
use std::{fs::DirEntry, io::Write, os::windows::process::CommandExt, path::PathBuf};

use clap::{command, Parser, Subcommand};
use config::{ConfigFile, Package};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(version, about = "A mini build system for Java", long_about = None)]
struct Args {
    #[command(subcommand)]
    target: Target,
}

#[derive(Subcommand, Clone)]
enum Target {
    Build {
        #[arg(long, required = false, num_args = 0, default_value = None, action = clap::ArgAction::Count)]
        jar: u8,
    },
    Run {
        #[arg(long, required = false, num_args = 0, default_value = None, action = clap::ArgAction::Count)]
        jar: u8,
    },
    Init {
        name: String,
    },
}

fn main() {
    let args = Args::parse();
    let res = match args.target {
        Target::Build { jar } => build(None, jar != 0),
        Target::Run { jar } => run(jar != 0),
        Target::Init { name } => init(name),
    };

    match res {
        Ok(()) => (),
        Err(e) => eprintln!("An error occured during target execution: {}", e),
    }
}

fn get_config() -> Result<config::ConfigFile> {
    use std::fs;
    let file = fs::read_to_string("./Nopain.toml")?;
    match toml::from_str(&file) {
        Ok(c) => Ok(c),
        Err(e) => Err(Box::new(e)),
    }
}

fn build(cfg: Option<ConfigFile>, jar: bool) -> Result<()> {
    let cfg = match cfg {
        Some(c) => c,
        None => get_config()?,
    };
    let working_dir = std::env::current_dir()?;
    let mut src_dir = working_dir.clone();
    src_dir.push("src");

    let sources = get_sources(&src_dir, "java")?;
    use std::process::Command;
    // let mut sources_string = String::new();

    let mut output = Command::new(&cfg.package.compiler);
    output.arg("-classpath");
    #[cfg(target_os = "windows")]
    output.arg(r#".;lib"#);
    #[cfg(target_os = "linux")]
    output.arg(r#".:lib"#);
    output.arg("-d");
    output.arg("bin");

    for (index, src) in sources.iter().enumerate() {
        if index == sources.len() - 1 {
            output.arg(format!("{}", src.path().to_str().unwrap()));
        } else {
            output.arg(format!("{};", src.path().to_str().unwrap()));
        }
    }
    let _output = output.output()?;
    if jar {
        jar_package(&cfg)?;
    }
    Ok(())
}

fn get_sources(path: &PathBuf, ext: &str) -> Result<Vec<DirEntry>> {
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

fn jar_package(cfg: &ConfigFile) -> Result<()> {
    let package = cfg.package.name.as_str();
    use std::process::Command;
    let mut output = Command::new(&cfg.package.jar);
    let mut current_dir = std::env::current_dir()?;
    current_dir.push("bin");
    output.current_dir(current_dir);
    let target = format!("../target/{}.jar", package);
    if let Some(main) = &cfg.package.main {
        output.arg("cfe");
        output.arg(target);
        output.arg(main.as_str());

        let mut main_path = String::from("");
        main_path.push_str(main.as_str());
        let mut main_path = main_path.replace("/", ".");
        main_path.push_str(".class");
        output.arg(main_path);
    } else {
        output.arg("cf");
        output.arg(target);
    }
    let out = output.output()?;
    std::io::stdout().write_all(&out.stdout)?;
    std::io::stderr().write_all(&out.stderr)?;
    Ok(())
}

fn run(jar: bool) -> Result<()> {
    let cfg = get_config()?;
    let Some(main) = &cfg.package.main else {
        return Err(Box::new(BuildError {
            msg: "This package contains no entry point class".to_owned(),
        }));
    };
    build(Some(cfg.clone()), jar)?;
    use std::process::Command;
    let mut output = Command::new(&cfg.package.java);
    
    output.arg("-classpath");
    #[cfg(target_os = "windows")]
    output.arg(r#"lib;bin"#);
    #[cfg(target_os = "linux")]
    output.arg(r#"lib:bin"#);
    if jar{
        jar_package(&cfg)?;
        output.arg("-jar");
        // let mut wd = std::env::current_dir()?;
        // wd.push("target");
        output.arg(&format!("target/{}.jar", cfg.package.name));
    }
    output.arg(main.as_str());

    let _output = output.output()?;

    std::io::stdout().write_all(&_output.stdout)?;
    std::io::stderr().write_all(&_output.stderr)?;

    Ok(())
}

#[derive(Debug, Clone)]
struct InitError {
    msg: String,
}

impl std::error::Error for InitError {}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.msg)
    }
}

fn init(name: String) -> Result<()> {
    use std::fs;
    if !name.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(Box::new(InitError {
            msg: format!("project name is invalid `{}`", name),
        }));
    }
    fs::DirBuilder::new().create(&name)?;

    let mut dir = PathBuf::new();
    dir.push(&name);

    for sub in ["src", "lib", "bin", "target"] {
        dir.push(sub);
        fs::DirBuilder::new().create(&dir)?;
        dir.pop();
    }

    let config = config::ConfigFile {
        package: config::Package {
            name: name.clone(),
            version: "0.0.1".to_owned(),
            #[cfg(target_os = "windows")]
            compiler: "javac".to_owned(),
            #[cfg(target_os = "windows")]
            java: "java".to_owned(),
            #[cfg(target_os = "windows")]
            jar: "jar".to_owned(),
            #[cfg(target_os = "linux")]
            compiler: "javac".to_owned(),
            #[cfg(target_os = "linux")]
            java: "java".to_owned(),
            #[cfg(target_os = "linux")]
            jar: "jar".to_owned(),
            main: "Main".to_owned().into(),
        },
        import: None,
    };

    dir.push("src/Main.java");
    let mut src = fs::File::create_new(&dir)?;
    src.write_all(
        r#"
public class Main{
    public static void main(String[] args){
        System.out.println("No pain, all gain!");
    }
}
    "#
        .as_bytes(),
    )?;
    dir.pop();
    dir.pop();

    let config = toml::to_string(&config)?;
    dir.push("Nopain.toml");
    let mut config_file = fs::File::create_new(&dir)?;
    config_file.write_all(config.as_bytes())?;

    Ok(())
}

#[derive(Debug)]
struct BuildError {
    msg: String,
}

impl std::error::Error for BuildError {}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}
