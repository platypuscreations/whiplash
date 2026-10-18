use std::env;
use std::path::PathBuf;

mod build_bindgen;
use crate::build_bindgen::bindgen;

// Functions to determine feature flags
fn enable_playlist() -> &'static str {
    if cfg!(feature = "playlist") {
        "ON"
    } else {
        "OFF"
    }
}

// Are we linking to shared or static libraries?
fn build_shared_libs_flag() -> &'static str {
    if cfg!(feature = "static") {
        "OFF" // Disable shared libs to enable static linking
    } else {
        "ON" // Enable shared libs
    }
}

fn main() {
    // Path to the projectM source code
    let projectm_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("libprojectM");

    // Verify the existence of the libprojectM directory
    if !projectm_path.exists() {
        println!("cargo:warning=The libprojectM source code is missing.");
        println!(
            "cargo:warning=If you are building from a git clone, please run 'git submodule update --init --recursive'."
        );
        println!("cargo:warning=If you downloaded this crate from crates.io, please ensure that the crate was packaged correctly.");
        std::process::exit(1);
    }

    // Determine feature flags
    let enable_playlist_flag = enable_playlist();
    let build_shared_libs = build_shared_libs_flag();

    let dst;

    // Platform-specific CMake configurations
    if cfg!(target_os = "windows") {
        // Ensure VCPKG installation root is set
        let vcpkg_root = match env::var("VCPKG_INSTALLATION_ROOT") {
            Ok(val) => val,
            Err(_) => {
                println!("cargo:warning=VCPKG_INSTALLATION_ROOT is not set. Please set it to your VCPKG installation directory.");
                std::process::exit(1);
            }
        };

        let vcpkg_root = PathBuf::from(vcpkg_root);
        let vcpkg_toolchain = vcpkg_root
            .join("scripts")
            .join("buildsystems")
            .join("vcpkg.cmake");

        if !vcpkg_toolchain.exists() {
            println!(
                "cargo:warning=The vcpkg toolchain file was not found at: {}",
                vcpkg_toolchain.display()
            );
            std::process::exit(1);
        }

        // Set VCPKG_ROOT for CMake
        env::set_var("VCPKG_ROOT", &vcpkg_root);

        // whiplash patch: match the C runtime Rust links. Rust always uses
        // the release runtime, even in debug builds, and links it statically
        // with `+crt-static`.
        let crt_static = env::var("CARGO_CFG_TARGET_FEATURE").is_ok_and(|f| f.split(',').any(|f| f == "crt-static"));
        let (triplet, msvc_runtime) =
            if crt_static { ("x64-windows-static", "MultiThreaded") } else { ("x64-windows-static-md", "MultiThreadedDLL") };

        // Convert vcpkg_toolchain to string
        let vcpkg_toolchain_str = vcpkg_toolchain.to_str().unwrap();

        // Configure and build libprojectM using CMake for Windows
        let mut cmake_config = cmake::Config::new(&projectm_path);
        cmake_config
            // whiplash patch: no fixed generator; the cmake crate picks the
            // Visual Studio that's installed (2022, 2026, ...).
            .define("CMAKE_TOOLCHAIN_FILE", vcpkg_toolchain_str)
            .define("VCPKG_TARGET_TRIPLET", triplet)
            .define("CMAKE_MSVC_RUNTIME_LIBRARY", msvc_runtime)
            .define("ENABLE_PLAYLIST", enable_playlist_flag)
            // whiplash patch: build the bundled projectm-eval and GLM into
            // the library, as on other systems; vcpkg then builds only GLEW.
            .define("ENABLE_SYSTEM_PROJECTM_EVAL", "OFF")
            .define("ENABLE_SYSTEM_GLM", "OFF")
            .define("ENABLE_DEBUG_POSTFIX", "OFF")
            .profile("Release")
            .define("CMAKE_VERBOSE_MAKEFILE", "ON")
            .define("BUILD_TESTING", "OFF")
            .define("BUILD_EXAMPLES", "OFF")
            .define("BUILD_SHARED_LIBS", build_shared_libs); // static/dynamic

        // whiplash patch: the playlist's object library doesn't inherit this
        // from projectM's static target, and without it imports projectM's
        // functions as if from a DLL (linker warning LNK4217).
        if cfg!(feature = "static") {
            cmake_config.cxxflag("-DPROJECTM_STATIC_DEFINE");
        }

        dst = cmake_config.build();

        // whiplash patch: GLEW, which projectM needs on Windows, built by
        // vcpkg's manifest mode into the build folder.
        println!(
            "cargo:rustc-link-search=native={}",
            dst.join("build").join("vcpkg_installed").join(triplet).join("lib").display()
        );
        println!("cargo:rustc-link-lib=static=glew32");
    } else if cfg!(target_os = "emscripten") {
        // Configure and build libprojectM using CMake for Emscripten
        dst = cmake::Config::new(&projectm_path)
            .define("ENABLE_DEBUG_POSTFIX", "OFF") // whiplash patch, as below
            .profile("Release")
            .define("ENABLE_PLAYLIST", enable_playlist_flag)
            .define("BUILD_TESTING", "OFF")
            .define("BUILD_EXAMPLES", "OFF")
            .define("ENABLE_EMSCRIPTEN", "ON")
            .define("BUILD_SHARED_LIBS", build_shared_libs) // static/dynamic
            .build();
    } else {
        // Configure and build libprojectM using CMake for other platforms (Linux, macOS)
        dst = cmake::Config::new(&projectm_path)
            // whiplash patch: always an optimized build, without a "d" on
            // the library names, whatever Cargo's profile.
            .define("ENABLE_DEBUG_POSTFIX", "OFF")
            .profile("Release")
            .define("ENABLE_PLAYLIST", enable_playlist_flag)
            .define("BUILD_TESTING", "OFF")
            .define("BUILD_EXAMPLES", "OFF")
            .define("BUILD_SHARED_LIBS", build_shared_libs) // static/dynamic
            .build();
    }

    // Specify the library search path
    println!("cargo:rustc-link-search=native={}/lib", dst.display());

    // whiplash patch: one set of library names, since projectM is always
    // built in its Release configuration. CMake puts "lib" in front of static
    // libraries' names on Windows.
    let prefix = if cfg!(target_os = "windows") && cfg!(feature = "static") { "lib" } else { "" };
    let kind = if cfg!(feature = "static") { "static" } else { "dylib" };
    println!("cargo:rustc-link-lib={kind}={prefix}projectM-4");
    if cfg!(feature = "playlist") {
        println!("cargo:rustc-link-lib={kind}={prefix}projectM-4-playlist");
    }

    // Platform-specific link flags for C++ and OpenGL
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=framework=OpenGL");
    }
    #[cfg(target_os = "linux")]
    {
        // On Linux, link stdc++ and GL.
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=GL");
        println!("cargo:rustc-link-lib=gomp");
    }
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=opengl32");
    }
    #[cfg(target_os = "emscripten")]
    {
        // Emscripten typically handles GL calls differently, so you might skip or rely on the
        // emscripten compiler for linking.
    }

    // Generate Rust bindings using bindgen
    bindgen();
}
