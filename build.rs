use std::{env, error::Error, fs, io, path::PathBuf, process::Command};

fn resource_compiler() -> io::Result<PathBuf> {
    if let Some(search_path) = env::var_os("PATH")
        && let Some(compiler) =
            env::split_paths(&search_path).map(|path| path.join("rc.exe")).find(|path| path.is_file())
    {
        return Ok(compiler);
    }

    let sdk_directory = env::var_os("WindowsSdkDir")
        .map(PathBuf::from)
        .or_else(|| env::var_os("ProgramFiles(x86)").map(|directory| PathBuf::from(directory).join("Windows Kits/10")));
    if let Some(sdk_directory) = sdk_directory {
        let mut versions = Vec::new();
        for entry in fs::read_dir(sdk_directory.join("bin"))? {
            let entry = entry?;
            let version =
                entry.file_name().to_string_lossy().split('.').map(str::parse::<u32>).collect::<Result<Vec<_>, _>>();
            if let Ok(version) = version {
                versions.push((version, entry.path().join("x64/rc.exe")));
            }
        }
        versions.sort_by(|left, right| right.0.cmp(&left.0));
        if let Some((_, compiler)) = versions.into_iter().find(|(_, compiler)| compiler.is_file()) {
            return Ok(compiler);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "rc.exe not found; install the Windows SDK with the Desktop development with C++ workload",
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=assets/branding/logo.rc");
    println!("cargo:rerun-if-changed=assets/branding/logo.ico");
    println!("cargo:rerun-if-env-changed=WindowsSdkDir");
    if env::var("CARGO_CFG_TARGET_OS")? != "windows" {
        return Ok(());
    }

    let resources = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is missing")?).join("logo.res");
    let status = Command::new(resource_compiler()?)
        .current_dir("assets/branding")
        .arg("/nologo")
        .arg("/fo")
        .arg(&resources)
        .arg("logo.rc")
        .status()?;
    if !status.success() {
        return Err(format!("Windows icon resource compilation failed: {status}").into());
    }
    println!("cargo:rustc-link-arg={}", resources.display());
    Ok(())
}
