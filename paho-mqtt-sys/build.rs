// paho-mqtt-sys/build.rs
//
// This file is part of the Eclipse Paho MQTT Rust Client library.
//

/*******************************************************************************
 * Copyright (c) 2017-2020 Frank Pagliughi <fpagliughi@mindspring.com>
 *
 * All rights reserved. This program and the accompanying materials
 * are made available under the terms of the Eclipse Public License v1.0
 * and Eclipse Distribution License v1.0 which accompany this distribution.
 *
 * The Eclipse Public License is available at
 *    http://www.eclipse.org/legal/epl-v10.html
 * and the Eclipse Distribution License is available at
 *   http://www.eclipse.org/org/documents/edl-v10.php.
 *
 * Contributors:
 *    Frank Pagliughi - initial implementation and documentation
 *******************************************************************************/

// Run 'cargo build -vv' to see debug output to console or check the 'output'
// file in the build directory.
//
// We write some strings "debug:..."
// This is helpful to figure out what the build is doing.

// To work with development branches of the C lib, the user can define
// environment variables:
//      PAHO_MQTT_C_INCLUDE_DIR= ...path to headers...
//      PAHO_MQTT_C_LIB_DIR= ...path to library...`
//
// or you could simply set:
//      PAHO_MQTT_C_DIR=...
//
// in which case it will assume:
//      PAHO_MQTT_C_INCLUDE_DIR=$PAHO_C_DIR/include
//      PAHO_MQTT_C_LIB_DIR=$PAHO_C_DIR/lib
//
// The basic decision tree is as follow:
//  + If "bundled" feature, compile the bundled C lib
//    - If "build_bindgen" feature, regenerate bindings
//    - else use bundled bindings
//  + else (!"bundled")
//    - If environment vars set use that lib,
//      ^ If "builtime_bindgen" generate bindings for user lib
//      ^ else assume proper version and use bundled bindings
//    - else (no env)
//      ^ If "builtime_bindgen" exit with an error
//      ^ else assume system install and use bundled bindings
//

use std::path::{Path, PathBuf};

// TODO: Assuming the proper installed version of the library is problematic.
//      We should check that the version is correct, if possible.

const PAHO_MQTT_C_VERSION: &str = "1.3.5";

fn main() {
    build::main();
}

// Determines the base name of which Paho C library we will link to.
// This is the name of the libary for the linker.
// Determine if we're usine SSL or not, by feature request.
fn link_lib_base() -> &'static str {
    if cfg!(feature = "ssl") {
        println!("debug:link Using SSL library");
        if cfg!(windows) {
            "paho-mqtt3as-static"
        } else {
            "paho-mqtt3as"
        }
    } else {
        println!("debug:link Using non-SSL library");
        if cfg!(windows) {
            "paho-mqtt3a-static"
        } else {
            "paho-mqtt3a"
        }
    }
}

// Try to find the Paho C library in one of the typical library
// directories under the specified install path.
// On success returns the path and base library name (i.e. the search path
// and link library name).
fn find_link_lib<P>(install_path: P) -> Option<(PathBuf, &'static str)>
where
    P: AsRef<Path>,
{
    let install_path = install_path.as_ref();
    let lib_dirs = &["lib", "lib64"];

    let lib_base = link_lib_base();

    let lib_file = if cfg!(windows) {
        format!("{}.lib", lib_base)
    } else {
        format!("lib{}.a", lib_base)
    };

    for dir in lib_dirs {
        let lib_path = install_path.join(dir);
        let lib = lib_path.join(&lib_file);

        if lib.exists() {
            return Some((lib_path, lib_base));
        }
    }
    None
}

// Here we're looking for some pre-generated bindings specific for the
// version of the C lib and the build target.
// If not found, it settles on the default bindings, giben the target
// word size (32 or 64-bit).
#[cfg(not(feature = "build_bindgen"))]
mod bindings {
    use super::*;
    use std::{env, fs};

    pub fn place_bindings(_inc_dir: &Path) {
        let target = env::var("TARGET").unwrap();
        println!("debug:Using existing Paho C binding for target: {}", target);

        let out_dir = env::var("OUT_DIR").unwrap();
        let out_path = Path::new(&out_dir).join("bindings.rs");

        let target_ptr_wd = env::var("CARGO_CFG_TARGET_POINTER_WIDTH").unwrap();
        println!("debug:Target Pointer Width: {}", target_ptr_wd);

        let mut bindings = format!(
            "bindings/bindings_paho_mqtt_c_{}-{}.rs",
            PAHO_MQTT_C_VERSION, target
        );

        if !Path::new(&bindings).exists() {
            println!(
                "No bindings exist for: {}. Using {}-bit default.",
                bindings, target_ptr_wd
            );
            bindings = format!(
                "bindings/bindings_paho_mqtt_c_{}-default-{}.rs",
                PAHO_MQTT_C_VERSION, target_ptr_wd
            )
        }

        println!("debug:Using bindings from: {}", bindings);
        fs::copy(&bindings, out_path).expect("Could not copy bindings to output directory");
    }
}

// Here we create new bindings using bindgen.
#[cfg(feature = "build_bindgen")]
mod bindings {
    extern crate bindgen;

    use super::*;
    use std::path::{Path, PathBuf};
    use std::{env, fs};

    pub fn place_bindings(inc_dir: &Path) {
        println!("debug:Using bindgen for Paho C");
        let cver = bindgen::clang_version();
        println!("debug:clang version: {}", cver.full);

        let inc_search = format!("-I{}", inc_dir.display());
        println!("debug:bindgen include path: {}", inc_search);

        // The bindgen::Builder is the main entry point
        // to bindgen, and lets you build up options for
        // the resulting bindings.
        let bindings = bindgen::Builder::default()
            // Older clang versions (~v3.6) improperly mangle the functions.
            // We shouldn't require mangling for straight C library.
            .trust_clang_mangling(false)
            // The input header we would like to generate
            // bindings for.
            .header("wrapper.h")
            .clang_arg(inc_search)
            // Finish the builder and generate the bindings.
            .generate()
            // Unwrap the Result and panic on failure.
            .expect("Unable to generate bindings");

        // Write the bindings to the $OUT_DIR/bindings.rs file.
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let out_path = out_dir.join("bindings.rs");

        bindings
            .write_to_file(out_path.clone())
            .expect("Couldn't write bindings!");

        // Save a copy of the bindings file into the bindings/ dir
        // with version and target name, if it doesn't already exist

        let target = env::var("TARGET").unwrap();
        println!("debug:Target: {}", target);

        let bindings = format!(
            "bindings/bindings_paho_mqtt_c_{}-{}.rs",
            PAHO_MQTT_C_VERSION, target
        );

        if !Path::new(&bindings).exists() {
            if let Err(err) = fs::copy(out_path, &bindings) {
                println!("debug:Error copying new binding file: {}", err);
            } else {
                println!("debug:Created new bindings file {}", bindings)
            }
        }
    }
}

// Here we're building with the bundled Paho C library.
// This will expand the Git submodule containing the C library, if it
// doesn't already exist, then it will run CMake on the library using
// configuration options that are useful for the Rust wrapper.
#[cfg(feature = "bundled")]
mod build {
    extern crate cmake;

    use super::*;
    use std::env;
    use std::path::Path;
    use std::process;
    use std::process::Command;

    pub fn main() {
        println!("debug:Running the bundled build for Paho C");

        // we rerun the build if the `build.rs` file is changed.
        println!("cargo:rerun-if-changed=build.rs");

        // Mske sure that the Git submodule is checked out

        if !Path::new("paho.mqtt.c/.git").exists() {
            let _ = Command::new("git")
                .args(&["submodule", "update", "--init"])
                .status();
        }

        // Use and configure cmake to build the Paho C lib
        let ssl = if cfg!(feature = "ssl") { "on" } else { "off" };

        let mut cmk_cfg = cmake::Config::new("paho.mqtt.c/");
        cmk_cfg
            .define("PAHO_BUILD_SHARED", "off")
            .define("PAHO_BUILD_STATIC", "on")
            .define("PAHO_ENABLE_TESTING", "off")
            .define("PAHO_HIGH_PERFORMANCE", "on")
            .define("PAHO_WITH_SSL", ssl);

        if cfg!(windows) {
            cmk_cfg.cflag("/DWIN32");
        }

        cmk_cfg.cflag("-std=c11");

        if let Ok(ssl_sp) = env::var("OPENSSL_ROOT_DIR") {
            cmk_cfg.define("OPENSSL_ROOT_DIR", format!("{}", ssl_sp));
        }

        // 'cmk_install_dir' is a PathBuf to the cmake install directory
        let cmk_install_path = cmk_cfg.build();
        println!("debug:CMake output dir: {}", cmk_install_path.display());

        let (lib_path, link_lib) = match find_link_lib(&cmk_install_path) {
            Some(lib) => lib,
            _ => {
                println!("Error building Paho C library.");
                process::exit(103);
            }
        };

        println!(
            "debug:Using Paho C library at: {} [{}]",
            lib_path.display(),
            link_lib
        );

        // Get bundled bindings or regenerate
        let inc_dir = cmk_install_path.join("include");
        println!("debug:Using Paho C headers at: {}", inc_dir.display());

        bindings::place_bindings(&inc_dir);

        // Link in the SSL libraries if configured for it.
        if cfg!(feature = "ssl") {
            if cfg!(windows) {
                println!("cargo:rustc-link-lib=libssl");
                println!("cargo:rustc-link-lib=libcrypto");
                if let Ok(ssl_sp) = env::var("OPENSSL_ROOT_DIR") {
                    println!("cargo:rustc-link-search={}\\lib", ssl_sp);
                } else {
                    #[cfg(target_arch = "x86")]
                    println!("cargo:rustc-link-search={}\\lib", "C:\\OpenSSL-Win32");

                    #[cfg(target_arch = "x86_64")]
                    println!("cargo:rustc-link-search={}\\lib", "C:\\OpenSSL-Win64");
                };
            } else {
                println!("cargo:rustc-link-lib=ssl");
                println!("cargo:rustc-link-lib=crypto");
                if let Ok(ssl_sp) = env::var("OPENSSL_ROOT_DIR") {
                    println!("cargo:rustc-link-search={}/lib", ssl_sp);
                }
            }
        }

        // we add the folder where all the libraries are built to the path search
        println!("cargo:rustc-link-search=native={}", lib_path.display());
        println!("cargo:rustc-link-lib=static={}", link_lib);
    }
}

// Here we're building with an existing Paho C library.
// This can be a library installed on the system or the location might be
// specified with some environment variables, like: "PAHO_MQTT_C_DIR"
#[cfg(not(feature = "bundled"))]
mod build {
    use super::*;
    use std::env;
    use std::path::Path;

    // Set the library path, and return the location of the header,
    // if found.
    fn find_paho_c() -> Option<String> {
        let link_lib = link_lib_base();

        println!("cargo:rerun-if-env-changed=PAHO_MQTT_C_DIR");
        println!("cargo:rerun-if-env-changed=PAHO_MQTT_C_INCLUDE_DIR");
        println!("cargo:rerun-if-env-changed=PAHO_MQTT_C_LIB_DIR");

        if cfg!(target_os = "windows") {
            println!("cargo:rerun-if-env-changed=PATH");
        }

        println!("debug:Building with existing library: {}", link_lib);
        println!("cargo:rustc-link-lib={}", link_lib);

        // Allow users to specify where to find the C lib.
        if let Ok(lib_dir) = env::var("PAHO_MQTT_C_LIB_DIR") {
            if let Ok(inc_dir) = env::var("PAHO_MQTT_C_INCLUDE_DIR") {
                println!("debug:inc_dir={}", inc_dir);
                println!("debug:lib_dir={}", lib_dir);

                println!("cargo:rustc-link-search={}", lib_dir);
                Some(inc_dir)
            } else {
                panic!("If specifying lib dir, must also specify include dir");
            }
        } else if let Ok(dir) = env::var("PAHO_MQTT_C_DIR") {
            if let Some((lib_path, _link_lib)) = find_link_lib(&dir) {
                println!("cargo:rustc-link-search={}", lib_path.display());
                Some(format!("{}/include", dir))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn main() {
        println!("debug:Running the un-bundled build for Paho C");

        // We will use the directory `paho.mqtt.c`, if something change
        // there we should rerun the compile step.
        println!("cargo:rerun-if-changed=paho.mqtt.c/");

        let inc_dir = find_paho_c().unwrap_or_default();
        if cfg!(feature = "build_bindgen") && inc_dir.is_empty() {
            panic!("Can't generate bindings. Unknown library location");
        }

        bindings::place_bindings(&Path::new(&inc_dir));
    }
}
