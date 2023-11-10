use glob::glob;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "macos")]
const GLSL_COMPILER_PATH: &str = "/Users/hahaliu/VulkanSDK/1.3.261.1/macOS/bin/glslc";
#[cfg(target_os = "windows")]
const GLSL_COMPILER_PATH: &str = "C:/VulkanSDK/1.3.261.1/Bin/glslc.exe";
const SHADER_SRC_DIRS: [&str; 2] = ["src/shaders", "examples/shaders"];

fn shader_paths_in_dir<P: AsRef<Path>>(dir: &P) -> Vec<PathBuf> {
    let mut shader_paths = vec![];

    for suffix in ["*.vert", "*.frag", "*.comp"] {
        for entry in glob(dir.as_ref().join("**").join(suffix).to_str().unwrap())
            .expect("Failed to read glob pattern")
        {
            shader_paths.push(entry.unwrap());
        }
    }

    shader_paths
}

fn compile_shader<P: AsRef<Path>>(compiler: P, shader: P) {
    println!("Compiling shader: {}", shader.as_ref().display());
    Command::new(compiler.as_ref())
        .args([
            shader.as_ref().to_str().unwrap(),
            "-o",
            &(shader.as_ref().to_str().unwrap().to_owned() + ".spv"),
        ])
        .output()
        .unwrap_or_else(|_| panic!("failed at compile shader: {}", shader.as_ref().display()));
    println!(
        "Compiling shader output: {}",
        shader.as_ref().to_str().unwrap().to_owned() + ".spv"
    );
}

fn compile_shaders() {
    let compiler = Path::new(&GLSL_COMPILER_PATH);
    assert!(
        compiler.exists(),
        "glsl compiler path {GLSL_COMPILER_PATH} not exists, please check"
    );

    SHADER_SRC_DIRS
        .iter()
        .for_each(|p| println!("cargo:rerun-if-changed={p}"));

    let shader_paths = SHADER_SRC_DIRS
        .iter()
        .map(|p| shader_paths_in_dir(&std::env::current_dir().unwrap().join(p)))
        .collect::<Vec<_>>()
        .concat();

    shader_paths
        .iter()
        .for_each(|s| compile_shader(compiler, s));
}

fn main() {
    println!("Runing build scripts");

    compile_shaders();
}
