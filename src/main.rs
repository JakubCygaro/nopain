mod config;
mod maintenance;
mod erros;

use erros::{ImportValidationError, BuildError, InitError};
use std::{io::Write, path::PathBuf};
use clap::{command, Parser, Subcommand};
use config::ConfigFile;

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
    match args.target {
        Target::Build { jar } => {
            match build(jar != 0) {
                Err(e) => eprintln!("Build error: {}", e),
                Ok(_) => println!("Build done"),
            }
        },
        Target::Run { jar } => {
            match run(jar != 0) {
                Err(e) => eprintln!("Run error: {}", e),
                Ok(_) => (),
            }
        },
        Target::Init { name } => {
            match init(name) {
                Err(e) => eprintln!("Init error: {}", e),
                Ok(_) => println!("Initialized project directory"),
            };
        },
    };
}



fn build(jar: bool) -> Result<PostBuildData> {
    use std::process::Command;

    let cfg = maintenance::get_config()?;
    let mut lockfile = maintenance::get_lock_file()?;
    let mut output = Command::new(&cfg.package.compiler);

    let working_dir = std::env::current_dir()?;
    let mut src_dir = working_dir.clone();
    
    //gather external libs
    let mut external_libs: Vec<PathBuf> = vec![];
    if let Some(import) = &cfg.import {
        for ext_lib in import {
            let path = PathBuf::from(&ext_lib.path);
            // let path = path.canonicalize()?;
            if path.extension().unwrap_or_default() != "jar" {
                return Err(Box::new(ImportValidationError{path: path}))
            }
            external_libs.push(path);
        }
    }

    //gather libs
    let mut lib_dir = working_dir.clone();
    lib_dir.push("lib");
    let libs = maintenance::get_sources(&lib_dir, "jar")?
        .into_iter()
        .map(|d| d.path())
        .map(|p| p.strip_prefix(&lib_dir).unwrap().to_owned())
        .collect::<Vec<_>>();

    //pass libs as arg
    output.arg("-classpath");
    let mut libs_arg = String::new();
    for lib in &libs {
        #[cfg(target_os = "windows")]
        libs_arg.push_str(&format!("lib/{};", lib.to_str().unwrap()));
        #[cfg(target_os = "linux")]
        libs_arg.push_str(&format!("lib/{}:", lib.to_str().unwrap()));
    }
    for ext_lib in &external_libs {
        #[cfg(target_os = "windows")]
        libs_arg.push_str(&format!("{};", ext_lib.to_str().unwrap()));
        #[cfg(target_os = "linux")]
        libs_arg.push_str(&format!("{}:", ext_lib.to_str().unwrap()));
    }

    //println!("lib_arg: {}", &libs_arg);
    output.arg(&libs_arg);

    //pass -d flag
    output.arg("-d");
    output.arg("bin");


    //gather sources
    src_dir.push("src");
    let sources = maintenance::get_sources(&src_dir, "java")?;
    let sources = sources.into_iter()
        .map(|d| d.path())
        .collect::<Vec<_>>();

    //pass recently modified sources
    for (_index, src) in sources.iter()
        .filter(|d| {
            let Some(last_build) = &lockfile.last_build else {
                return true;
            };
            match d.metadata() {
                Err(_) => false,
                Ok(m) => {
                    let m = m.modified().unwrap();
                    m.ge(last_build)
                }
            }
        })
        .enumerate() {
        let source_arg = format!("{}", src.to_str().unwrap());
        //println!("adding source:`{}`", &source_arg);
        output.arg(source_arg);
    }
    //run build
    let _output = output.output()?;
    
    std::io::stdout().write_all(&_output.stdout)?;
    std::io::stderr().write_all(&_output.stderr)?;

    //gather class files
    let classes = sources
        .into_iter()
        .map(|mut p| {
            p.set_extension("class");
            p.strip_prefix(&src_dir).unwrap().to_owned()
        })
        .collect::<Vec<PathBuf>>();
    let mut bin_dir = working_dir.clone();
    bin_dir.push("bin");

    lockfile.last_build = Some(std::time::SystemTime::now());
    maintenance::create_lock_file(&lockfile)?;

    let post_build = PostBuildData {
        cfg: cfg,
        libs: libs,
        classes,
        src_dir: src_dir,
        lib_dir: lib_dir,
        libs_arg: libs_arg,
        bin_dir: bin_dir,
        external_libs: external_libs,
        current_lock: lockfile
    };
    if jar {
        jar_package(&post_build.cfg, &post_build)?;
    }

    //make jar

    Ok(post_build)
}



fn jar_package(cfg: &ConfigFile, post_build: &PostBuildData) -> Result<()> {
    use std::process::Command;
    use std::fs;

    let package = cfg.package.name.as_str();
    let mut output = Command::new(&cfg.package.jar);

    //set current dir to 'bin/'
    let mut build_dir = std::env::current_dir()?;
    build_dir.push("target");
    build_dir.push("build");
    std::fs::DirBuilder::new()
        .recursive(true)
        .create(&build_dir)?;
    let target = format!("target/build/{}.jar", package);

    output.arg("cfm");
    output.arg(target);
    let manifest_path = generate_manifest(cfg, &post_build.libs, &post_build.external_libs)?;
    output.arg(manifest_path);
    output.arg("-C");
    output.arg("bin");
    output.arg(".");

    let out = output.output()?;

    build_dir.push("lib");
    let build_lib_dir = build_dir;
    fs::DirBuilder::new()
        .recursive(true)
        .create(&build_lib_dir)?;

    for lib in post_build.libs.iter()
        .map(|l| {
            post_build.lib_dir.join(l).to_owned()
        })
        
    {
        let dest = build_lib_dir.join(lib.file_name().unwrap());
        fs::copy(&lib, dest)?;
    }

    for ext in post_build.external_libs.iter()
    {
        let dest = build_lib_dir.join(ext.file_name().unwrap());
        fs::copy(&ext, dest)?;
    }
    

    std::io::stdout().write_all(&out.stdout)?;
    std::io::stderr().write_all(&out.stderr)?;
    Ok(())
}

fn run(jar: bool) -> Result<()> {
    use std::process::Command;

    let build_data = build(jar)?;
    let cfg = build_data.cfg;
    let Some(main) = &cfg.package.main else {
        return Err(Box::new(BuildError {
            msg: "This package contains no entry point class".to_owned(),
        }));
    };
    
    let mut output = Command::new(&cfg.package.java);

    // let class_files = build_data.classes.into_iter()
    //     .map(|p |{
    //         build_data.bin_dir.join(p)
    //     })
    //     .collect::<Vec<_>>();

    // pass -cp flag
    output.arg("-classpath");
    output.arg(format!("bin;{}", &build_data.libs_arg));

    if jar {
        output.arg("-jar");
        output.arg(&format!("target/build/{}.jar", cfg.package.name));
    } else {
        output.arg(main.as_str());
    }


    let _output = output.output()?;

    std::io::stdout().write_all(&_output.stdout)?;
    std::io::stderr().write_all(&_output.stderr)?;

    Ok(())
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

fn generate_manifest(cfg: &ConfigFile, libs: &Vec<PathBuf>, external_libs: &Vec<PathBuf>) -> Result<PathBuf>{
    use std::fs;
    let path = PathBuf::from("target/Manifest.txt");
    let mut manifest = fs::File::create(&path)?;
    
    write!(manifest, "Manifest-Version: 1.0\n")?;
    if let Some(entry) = &cfg.package.main {
        write!(manifest, "Main-Class: {}\n", entry)?;
    }
    write!(manifest, "Class-Path: ")?;
    for lib in libs {
        write!(manifest, "lib/{} ", lib.to_str().unwrap())?;
    }
    for ext in external_libs {
        write!(manifest, "lib/{} ", ext.file_name().unwrap().to_str().unwrap())?;
    }
    write!(manifest, "\n\n")?;
    Ok(path)
}
struct PostBuildData {
    /// all .jar file paths used for compilation
    pub libs: Vec<PathBuf>,
    /// the argument passed to -cp
    pub libs_arg: String,
    pub cfg: ConfigFile,
    /// a collection of .class files created after compilation
    pub classes: Vec<PathBuf>,
    pub bin_dir: PathBuf,
    pub src_dir: PathBuf,
    pub lib_dir: PathBuf,
    pub external_libs: Vec<PathBuf>,
    pub current_lock: config::NopainLock,
}


