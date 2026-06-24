fn main() {
    // AMD GPU telemetry for the metrics overlay: compile the C++ ADLX shim and
    // AMD's vendored SDK helper into a static lib linked into the binary. ADLX
    // loads amdadlx64.dll at runtime, so there's no import lib to link; on a
    // non-AMD machine `adlx_init` just fails and the overlay omits GPU metrics.
    #[cfg(windows)]
    {
        let adlx = "third_party/adlx";
        println!("cargo:rerun-if-changed=third_party/adlx_shim.cpp");
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
