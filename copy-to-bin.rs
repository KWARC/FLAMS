use std::path::Path;
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(&entry.path(), &dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
fn main() {
    let _ = std::fs::remove_dir_all(Path::new("./bin"));
    std::fs::create_dir_all(Path::new("./bin")).unwrap();
    copy_dir_all(&Path::new("./target/web"), &Path::new("./bin/web")).unwrap();
    std::fs::copy(
        Path::new("./resources/settings.toml"),
        Path::new("./bin/settings.toml"),
    )
    .unwrap();
    #[cfg(target_os = "windows")]
    {
        let main_file = Path::new("./target/x86_64-pc-windows-msvc/flams-release/flams.exe");
        if main_file.exists() {
            std::fs::copy(main_file, Path::new("./bin/flams.exe")).unwrap();
            std::fs::copy(
                Path::new("./target/x86_64-pc-windows-msvc/flams-release/hash.txt"),
                Path::new("./bin/hash.txt"),
            )
            .unwrap();
        } else {
            std::fs::copy(
                Path::new("./target/flams-release/flams.exe"),
                Path::new("./bin/flams.exe"),
            )
            .unwrap();
            std::fs::copy(
                Path::new("./target/flams-release/hash.txt"),
                Path::new("./bin/hash.txt"),
            )
            .unwrap();
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::fs::copy(
            Path::new("./target/flams-release/flams"),
            Path::new("./bin/flams"),
        )
        .unwrap();
        std::fs::copy(
            Path::new("./target/flams-release/hash.txt"),
            Path::new("./bin/hash.txt"),
        )
        .unwrap();
    }
}
