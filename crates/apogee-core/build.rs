//! Build script for apogee-core.
//!
//! When the `hwm14` feature is enabled, compiles the vendored NRL HWM14
//! Fortran source and C-ABI wrapper into a static library and links the
//! gfortran runtime. A working `gfortran` is required in that case.
//!
//! Without the feature the build script is a no-op (aside from the
//! `rerun-if-changed` guards), so the crate builds on systems without a
//! Fortran compiler.

fn main() {
    // Always re-run if the build script itself changes.
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(not(feature = "hwm14"))]
    {}

    #[cfg(feature = "hwm14")]
    {
        compile_hwm14();
    }
}

#[cfg(feature = "hwm14")]
fn compile_hwm14() {
    use std::env;
    use std::path::PathBuf;
    use std::process::Command;
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let vendor = PathBuf::from(&manifest_dir)
        .join("external")
        .join("hwm14")
        .join("vendor");

    // Compile the NRL HWM14 Fortran source and the C-ABI wrapper.
    let hwm14_src = vendor.join("hwm14.f90");
    let wrapper_src = vendor.join("hwm14_c.f90");
    let hwm14_obj = out_dir.join("hwm14.o");
    let wrapper_obj = out_dir.join("hwm14_c.o");
    let static_lib = out_dir.join("libhwm14.a");

    fortran_compile(
        &hwm14_src,
        &hwm14_obj,
        "-O2",
        "Failed to compile HWM14 Fortran source",
    );
    fortran_compile(
        &wrapper_src,
        &wrapper_obj,
        "-O2",
        "Failed to compile HWM14 C-ABI wrapper",
    );

    // Build a static library from the object files.
    let ar = env::var("AR").unwrap_or_else(|_| "ar".into());
    let status = Command::new(&ar)
        .args([
            "rcs",
            static_lib.to_str().unwrap(),
            hwm14_obj.to_str().unwrap(),
            wrapper_obj.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run archiver");
    if !status.success() {
        panic!("Failed to create libhwm14.a");
    }

    // Link the static library and the gfortran runtime.
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=hwm14");
    println!("cargo:rustc-link-lib=gfortran");
    println!("cargo:rustc-link-lib=quadmath");

    // Re-run build if the Fortran source or wrapper changes.
    println!("cargo:rerun-if-changed={}", hwm14_src.display());
    println!("cargo:rerun-if-changed={}", wrapper_src.display());
}

#[cfg(feature = "hwm14")]
fn fortran_compile(src: &std::path::Path, obj: &std::path::Path, flags: &str, err_msg: &str) {
    use std::env;
    use std::process::Command;
    let fc = env::var("FC").unwrap_or_else(|_| "gfortran".into());
    let out_dir = env::var("OUT_DIR").unwrap();
    let status = Command::new(&fc)
        .args([
            "-c",
            "-fPIC",
            flags,
            // Redirect .mod files to OUT_DIR so they don't pollute the
            // crate root.
            &format!("-J{out_dir}"),
            "-o",
            obj.to_str().unwrap(),
            src.to_str().unwrap(),
        ])
        .status()
        .unwrap_or_else(|_| panic!("{} (is `{}` installed?)", err_msg, fc));
    if !status.success() {
        panic!("{}", err_msg);
    }
}
