fn main() {
    // AMD GPU telemetry for the metrics overlay: compile the C++ ADLX shim and
    // AMD's vendored SDK helper into a static lib linked into the binary. ADLX
    // loads amdadlx64.dll at runtime, so there's no import lib to link; on a
    // non-AMD machine `adlx_init` just fails and the overlay omits GPU metrics.
    #[cfg(windows)]
    {
        let adlx = "third_party/adlx";
        // `cc` does not emit `rerun-if-changed` for the files it compiles, and
        // declaring *any* rerun rule switches cargo from "watch everything" to
        // "watch only these". Listing just the shim meant edits to the SDK
        // sources or headers silently linked a stale adlx_shim.lib.
        println!("cargo:rerun-if-changed=third_party/adlx_shim.cpp");
        println!("cargo:rerun-if-changed={adlx}/SDK/ADLXHelper/Windows/Cpp/ADLXHelper.cpp");
        println!("cargo:rerun-if-changed={adlx}/SDK/Platform/Windows/WinAPIs.cpp");
        println!("cargo:rerun-if-changed={adlx}/SDK/Include");
        cc::Build::new()
            .cpp(true)
            .include(adlx)
            .file("third_party/adlx_shim.cpp")
            .file(format!("{adlx}/SDK/ADLXHelper/Windows/Cpp/ADLXHelper.cpp"))
            .file(format!("{adlx}/SDK/Platform/Windows/WinAPIs.cpp"))
            .flag_if_supported("/EHsc")
            // ADLX SDK headers are third-party; don't fail our build on their warnings.
            .warnings(false)
            .compile("adlx_shim");
    }

    tauri_build::build()
}
