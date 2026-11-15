// Compiles the LXR V2 sound engine from the firmware sources in engine/firmware, with engine/shim
// standing in for the STM32 HAL, plus the plugin bridge in engine/, into a dylib that the plugin
// embeds. The firmware keeps its state in C globals, so every plugin instance loads its own copy of
// that dylib (src/engine.rs).
use std::path::{Path, PathBuf};

// Firmware sources that touch hardware the plugin doesn't have (codec, trigger outs, UART/USB MIDI,
// front panel link). lxr_stubs.c covers their symbols.
const EXCLUDE: &[&str] = &[
    "Soundengine/Soundengine.c",
    "Soundengine/cs4344_cs5343.c",
    "Soundengine/AudioCodecManager.c",
    "Soundengine/TriggerOut.c",
    "Soundengine/USB/",
    "Soundengine/MIDI/Uart.c",
    "Soundengine/MIDI/CortexParser.c",
];

fn c_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            c_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "c") {
            out.push(path);
        }
    }
}

fn main() {
    let engine = Path::new("engine");
    let core = engine.join("firmware/Core");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let windows = target_os == "windows";

    let mut sources = Vec::new();
    c_files(&core.join("Src/Soundengine"), &mut sources);
    sources.retain(|p| {
        let s = p.to_string_lossy().replace('\\', "/");
        !EXCLUDE.iter().any(|e| s.contains(e))
    });
    sources.sort();
    sources.push(core.join("Src/Preset/initKit.c"));
    sources.push(core.join("Src/menu/Cc2Text.c"));
    sources.push(engine.join("lxr_bridge.c"));
    sources.push(engine.join("lxr_entropia.c"));
    sources.push(engine.join("lxr_stubs.c"));

    let mut build = cc::Build::new();
    build
        .files(&sources)
        .include(engine.join("shim"))
        .include(core.join("Inc"))
        .include(core.join("Inc/Soundengine"))
        .include(core.join("Inc/Soundengine/DSPAudio"))
        .flag("-std=gnu11")
        .flag("-include")
        .flag(engine.join("shim/stm32f7xx_hal.h").to_str().unwrap())
        // the firmware places code/data in TCM RAM sections; there is no such thing here
        .define("section(x)", "used")
        .opt_level(3)
        .warnings(false)
        .flag_if_supported("-w")
        // not linked into the plugin itself
        .cargo_metadata(false);
    if !windows {
        // same minimum macOS as the Rust side links for
        build.flag("-mmacosx-version-min=11.0");
    }
    build.compile("lxr_engine");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let cc = std::env::var("CC")
        .or_else(|_| std::env::var("CC_x86_64_pc_windows_gnu"))
        .unwrap_or_else(|_| "cc".to_string());

    if windows {
        // Windows: the engine becomes a DLL (embed.rs loads a private copy per instance). Zig's clang
        // links it; --whole-archive keeps every translation unit, --export-all-symbols exports the API.
        let status = std::process::Command::new(&cc)
            .arg("-shared")
            .arg("-o")
            .arg(out.join("lxr_engine.dll"))
            .arg("-Wl,--whole-archive")
            .arg(out.join("liblxr_engine.a"))
            .arg("-Wl,--no-whole-archive")
            .args(["-Wl,--export-all-symbols", "-lm", "-lmingw32"])
            .status()
            .expect("run cc");
        assert!(status.success(), "linking lxr_engine.dll failed");
    } else {
        // built once per architecture; bundle-universal lipos the plugins together
        let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
            "aarch64" => "arm64",
            other => other,
        }
        .to_string();
        let status = std::process::Command::new("cc")
            .args(["-dynamiclib", "-arch", &arch, "-mmacosx-version-min=11.0", "-o"])
            .arg(out.join("lxr_engine.dylib"))
            .arg(format!("-Wl,-force_load,{}", out.join("liblxr_engine.a").display()))
            .args(["-install_name", "@rpath/lxr_engine.dylib", "-lm"])
            .status()
            .expect("run cc");
        assert!(status.success(), "linking lxr_engine.dylib failed");
    }

    println!("cargo:rerun-if-changed=engine");
}
