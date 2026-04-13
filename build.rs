use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 为本地 `libmpv` 运行时准备链接搜索路径和导入库。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - 无
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=local/libmpv-2.dll");

    if !cfg!(target_os = "windows") {
        return;
    }

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must exist"));
    let dll_path = manifest_dir.join("local").join("libmpv-2.dll");
    if !dll_path.exists() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must exist"));
    let link_dir = out_dir.join("libmpv-link");
    fs::create_dir_all(&link_dir).expect("failed to create libmpv link directory");

    let lib_path = link_dir.join("mpv.lib");
    generate_import_library(&dll_path, &lib_path);
    println!("cargo:rustc-link-search=native={}", link_dir.display());

    copy_runtime_dll(&dll_path, &out_dir);
}

/// 根据本地 DLL 导出表生成 MSVC 可用的导入库。
///
/// # 参数
/// - `dll_path`：本地 `libmpv` DLL 路径
/// - `lib_path`：目标导入库输出路径
///
/// # 返回值
/// - 无
fn generate_import_library(dll_path: &Path, lib_path: &Path) {
    let dumpbin = find_vs_tool("dumpbin.exe").expect("dumpbin.exe not found");
    let lib_exe = find_vs_tool("lib.exe").expect("lib.exe not found");
    let def_path = lib_path.with_extension("def");

    let exports = collect_exports(&dumpbin, dll_path);
    let mut def_contents = String::from("LIBRARY libmpv-2.dll\nEXPORTS\n");
    for export in exports {
        def_contents.push_str("    ");
        def_contents.push_str(&export);
        def_contents.push('\n');
    }
    fs::write(&def_path, def_contents).expect("failed to write mpv.def");

    let output = Command::new(lib_exe)
        .arg(format!("/def:{}", def_path.display()))
        .arg("/machine:x64")
        .arg(format!("/out:{}", lib_path.display()))
        .output()
        .expect("failed to run lib.exe");

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("failed to build mpv.lib\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }
}

/// 从 DLL 导出表提取 `mpv_*` 符号。
///
/// # 参数
/// - `dumpbin`：`dumpbin.exe` 路径
/// - `dll_path`：待扫描的 DLL 路径
///
/// # 返回值
/// - `Vec<String>`：排序后的导出符号名列表
fn collect_exports(dumpbin: &Path, dll_path: &Path) -> Vec<String> {
    let output = Command::new(dumpbin)
        .arg("/EXPORTS")
        .arg(dll_path)
        .output()
        .expect("failed to run dumpbin.exe");

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("failed to read DLL exports\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut exports = stdout
        .lines()
        .filter_map(|line| {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.len() < 4 {
                return None;
            }
            if !parts[0].chars().all(|ch| ch.is_ascii_digit()) {
                return None;
            }
            let name = parts[3];
            name.starts_with("mpv_").then(|| name.to_string())
        })
        .collect::<Vec<_>>();
    exports.sort_unstable();
    exports.dedup();
    exports
}

/// 查找 Visual Studio 自带工具。
///
/// # 参数
/// - `tool_name`：工具文件名
///
/// # 返回值
/// - `Option<PathBuf>`：找到时返回工具绝对路径
fn find_vs_tool(tool_name: &str) -> Option<PathBuf> {
    let program_files = env::var("ProgramFiles").ok()?;
    let base = PathBuf::from(program_files)
        .join("Microsoft Visual Studio")
        .join("2022");
    let editions = fs::read_dir(base).ok()?;

    for edition in editions.filter_map(Result::ok) {
        let msvc_root = edition.path().join("VC").join("Tools").join("MSVC");
        let versions = match fs::read_dir(msvc_root) {
            Ok(versions) => versions,
            Err(_) => continue,
        };

        for version in versions.filter_map(Result::ok) {
            let candidate = version
                .path()
                .join("bin")
                .join("Hostx64")
                .join("x64")
                .join(tool_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// 将运行时 DLL 复制到 cargo 的输出目录，避免测试/运行时找不到动态库。
///
/// # 参数
/// - `dll_path`：本地 `libmpv` DLL 路径
/// - `out_dir`：cargo 构建输出目录
///
/// # 返回值
/// - 无
fn copy_runtime_dll(dll_path: &Path, out_dir: &Path) {
    let Some(profile_dir) = out_dir.ancestors().nth(3) else {
        return;
    };
    let targets = [
        profile_dir.join("libmpv-2.dll"),
        profile_dir.join("deps").join("libmpv-2.dll"),
    ];

    for target in targets {
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::copy(dll_path, target);
    }
}
