mod config;
mod erros;
mod maintenance;
use clap::{command, Parser, Subcommand};
use colored::Colorize;
use config::ConfigFile;
use erros::{BuildError, ImportValidationError, InitError};
use log::{debug, error, info, trace, warn, Level, LevelFilter};
use std::collections::HashSet;
use std::fs;
use std::{io::Write, path::PathBuf};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(version, about = "A mini build system for Java", long_about = None)]
struct Args {
    #[arg(short, long, required = false, num_args = 0, default_value = None)]
    verbose: bool,
    #[command(subcommand)]
    target: Target,
}

#[derive(Subcommand, Clone)]
enum Target {
    Build {
        #[arg(short, long, required = false, num_args = 0, default_value = None)]
        jar: bool,
        #[arg(short, long, required = false, num_args = 1, default_value = None)]
        release: Option<i32>
    },
    Run {
        #[arg(short, long, required = false, num_args = 0, default_value = None)]
        jar: bool,
        #[arg(short, long, required = false, num_args = 1, default_value = None)]
        release: Option<i32>
    },
    Init {
        name: String,
    },
}

fn main() {
    let args = Args::parse();

    env_logger::builder()
        .format_timestamp(None)
        .filter_level(
            args.verbose
                .then(|| LevelFilter::Trace)
                .or(Some(LevelFilter::Info))
                .unwrap(),
        )
        .format_module_path(false)
        .init();

    match args.target {
        Target::Build { jar, release } => match build(jar, release) {
            Err(e) => error!("Build error: {}", e),
            Ok(_) => info!("Build done"),
        },
        Target::Run { jar, release } => match run(jar, release) {
            Err(e) => error!("Run error: {}", e),
            Ok(_) => (),
        },
        Target::Init { name } => {
            match init(name) {
                Err(e) => error!("Init error: {}", e),
                Ok(_) => info!("Initialized project directory"),
            };
        }
    };
}

fn build(jar: bool, release: Option<i32>) -> Result<PostBuildData> {
    use std::process::Command;
    info!("Starting build...");
    if let Some(release) = release {
        info!("Building for release {}", release);
    }
    let cfg = maintenance::get_config()?;
    let mut lockfile = maintenance::get_lock_file()?;
    let mut output = Command::new(&cfg.package.compiler);

    let working_dir = std::env::current_dir()?;
    let mut src_dir = working_dir.clone();
    let mut bin_dir = working_dir.clone();
    bin_dir.push("bin");
    //gather external libs
    trace!("{} external libraries", "Gathering".green().bold());
    let mut external_libs: Vec<PathBuf> = vec![];
    if let Some(import) = &cfg.import {
        for ext_lib in import {
            let path = PathBuf::from(&ext_lib.path);
            // let path = path.canonicalize()?;
            if path.extension().unwrap_or_default() != "jar" {
                return Err(Box::new(ImportValidationError { path: path }));
            }
            external_libs.push(path);
        }
    }

    //gather libs
    trace!("{} included libraries", "Gathering".green().bold());
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
        trace!("\t{} {:?}", "Including".blue().bold(), lib);

        #[cfg(target_os = "windows")]
        libs_arg.push_str(&format!("lib/{};", lib.to_str().unwrap()));
        #[cfg(target_os = "linux")]
        libs_arg.push_str(&format!("lib/{}:", lib.to_str().unwrap()));
    }
    for ext_lib in &external_libs {
        trace!("\t{} {:?}", "Including".blue().bold(), ext_lib);

        #[cfg(target_os = "windows")]
        libs_arg.push_str(&format!("{};", ext_lib.to_str().unwrap()));
        #[cfg(target_os = "linux")]
        libs_arg.push_str(&format!("{}:", ext_lib.to_str().unwrap()));
    }

    debug!("lib_arg: {}", &libs_arg);
    output.arg(&libs_arg);

    //add the --release flag
    if let Some(release) = release {
        output.arg("--release");
        output.arg(&format!("{}", release));
    }

    //pass -d flag
    output.arg("-d");
    output.arg("bin");


    //gather sources
    trace!("{} source files", "Gathering".green().bold());
    src_dir.push("src");
    let sources = maintenance::get_sources(&src_dir, "java")?;
    let sources = sources.into_iter().map(|d| d.path()).collect::<Vec<_>>();

    let mut source_count = 0;
    //pass recently modified sources
    for src in sources.iter()
    // The java compiler is the most retarded piece of work I've seen in a long time
    
    //     .filter(|d| {
    //     return true;
    //     let Some(last_build) = &lockfile.last_build else {
    //         return true;
    //     };

    //     if let Some(_) = release {
    //         return true;
    //     }

    //     //check if the corresponding .class file already exists
    //     let p = d.to_owned().clone();
    //     let p = p.strip_prefix(&src_dir).unwrap().to_owned();
    //     let mut p = bin_dir.join(p);
    //     p.set_extension("class");
    //     if let Ok(false) = p.try_exists() {
    //         debug!("`{:?}` does not exist", p);
    //         return true;
    //     }

    //     match d.metadata() {
    //         Err(_) => false,
    //         Ok(m) => {
    //             let m = m.modified().unwrap();
    //             m.ge(last_build)
    //         }
    //     }
    // }) 
    {
        let source_arg = format!("{}", src.to_str().unwrap());
        trace!("\t{} `{}`","Adding".green().bold(), &source_arg);
        output.arg(source_arg);
        source_count += 1;
    }
    //run build
    info!("Running build...");
    if source_count > 0 {
        let _output = output.output()?;
        if !_output.status.success(){
            std::io::stderr().write_all(&_output.stderr)?;
            return Err(Box::new(BuildError { msg: "Compilation failed".into() }))
        }
        std::io::stdout().write_all(&_output.stdout)?;
        info!("Compilation success");
    } else {
        info!("No need for compilation, all files up to date");
    }

    //gather class files
    let classes = sources
        .into_iter()
        .map(|mut p| {
            p.set_extension("class");
            p.strip_prefix(&src_dir).unwrap().to_owned()
        })
        .collect::<Vec<PathBuf>>();

    maintenance::purge_unused_classes(classes.clone())?;

    let old_lock = lockfile.clone();
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
        current_lock: lockfile,
    };
    //make jar
    if jar {
        package_jar(&post_build.cfg, &post_build, old_lock)?;
    }
    Ok(post_build)
}

fn package_jar(
    cfg: &ConfigFile,
    post_build: &PostBuildData,
    lock: config::NopainLock,
) -> Result<()> {
    use std::fs;
    use std::process::Command;
    info!("Starting jar action...");
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

    info!("Making jar");
    let out = output.output()?;

    if !out.status.success() {
        std::io::stderr().write_all(&out.stderr)?;
        return Err(Box::new(erros::JarError{ msg: "Failed to make the jar file".into() }));
    }

    build_dir.push("lib");
    let build_lib_dir = build_dir;
    fs::DirBuilder::new()
        .recursive(true)
        .create(&build_lib_dir)?;

    //copy libs from /lib to /target/build/lib
    trace!("{} included libraries to target/build/lib", "Copying".yellow().bold());
    for lib in post_build
        .libs
        .iter()
        .map(|l| post_build.lib_dir.join(l).to_owned())
    {
        let dest = build_lib_dir.join(lib.file_name().unwrap());
        let meta = lib.metadata()?;

        if let Some(last_build) = &lock.last_build {
            if !dest.exists() || meta.modified()?.ge(last_build) {
                fs::copy(&lib, dest)?;
            }
        } else {
            if !dest.exists() {
                fs::copy(&lib, dest)?;
            }
        }
    }
    trace!("{} external libraries to target/build/lib", "Copying".yellow().bold());
    
    for ext in post_build.external_libs.iter() {
        let dest = build_lib_dir.join(ext.file_name().unwrap());
        let meta = ext.metadata()?;

        if let Some(last_build) = &lock.last_build {
            if !dest.exists() || meta.modified()?.ge(last_build) {
                fs::copy(&ext, dest)?;
            }
        } else {
            if !dest.exists() {
                fs::copy(&ext, dest)?;
            }
        }
    }
    std::io::stdout().write_all(&out.stdout)?;
    Ok(())
}

fn run(jar: bool, release: Option<i32>) -> Result<()> {
    use std::process::Command;

    let build_data = build(jar, release)?;
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

fn generate_manifest(
    cfg: &ConfigFile,
    libs: &Vec<PathBuf>,
    external_libs: &Vec<PathBuf>,
) -> Result<PathBuf> {
    info!("Generating manifest file at `target/Manifest.txt`");
    use std::fs;
    let path = PathBuf::from("target/Manifest.txt");
    let mut manifest = fs::File::create(&path)?;
    trace!("{} to manifest file", "Writing".green().bold());

    write!(manifest, "Manifest-Version: 1.0\n")?;
    if let Some(entry) = &cfg.package.main {
        write!(manifest, "Main-Class: {}\n", entry)?;
    }
    write!(manifest, "Class-Path: ")?;
    for lib in libs {
        write!(manifest, "lib/{} ", lib.to_str().unwrap())?;
    }
    for ext in external_libs {
        write!(
            manifest,
            "lib/{} ",
            ext.file_name().unwrap().to_str().unwrap()
        )?;
    }
    write!(manifest, "\n\n")?;
    trace!("{} manifest generation", "Finished".yellow().bold());
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
