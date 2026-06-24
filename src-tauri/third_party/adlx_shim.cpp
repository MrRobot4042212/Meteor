// C-ABI shim over AMD's ADLX SDK, so Rust can read GPU telemetry on AMD cards
// (the NVML path only covers NVIDIA). ADLX dynamically loads `amdadlx64.dll`
// shipped with the Adrenalin driver; if it's absent/non-AMD, init fails and the
// caller falls back to "no GPU metrics" — same graceful degradation as NVML.
//
// Everything is driven from a single thread (the metrics sampler): ADLX is not
// called concurrently. Interfaces are kept alive in globals between samples.

#include "SDK/ADLXHelper/Windows/Cpp/ADLXHelper.h"
#include "SDK/Include/IPerformanceMonitoring3.h"

using namespace adlx;

// Validity bits for AdlxSample.flags — a field is only meaningful if its bit is set.
enum {
    ADLX_F_USAGE      = 1u << 0,
    ADLX_F_TEMP       = 1u << 1,
    ADLX_F_HOTSPOT    = 1u << 2,
    ADLX_F_POWER      = 1u << 3,
    ADLX_F_CLOCK      = 1u << 4,
    ADLX_F_VRAM_CLOCK = 1u << 5,
    ADLX_F_FAN        = 1u << 6,
    ADLX_F_VRAM_USED  = 1u << 7,
    ADLX_F_VRAM_TOTAL = 1u << 8,
    ADLX_F_FPS        = 1u << 9,
};

extern "C" {

// Flat snapshot handed back to Rust. Mirrors `AdlxSample` in `amd.rs`.
struct AdlxSample {
    double   gpu_usage;       // %
    double   gpu_temp;        // edge temperature, °C
    double   gpu_hotspot;     // junction/hotspot temperature, °C
    double   gpu_power;       // W (total board power preferred, else GPU power)
    int      gpu_clock;       // MHz
    int      vram_clock;      // MHz
    int      fan_rpm;         // RPM
    int      fps;             // frames/s (system-wide, focused app)
    unsigned vram_used_mb;    // MB
    unsigned vram_total_mb;   // MB
    unsigned flags;           // ADLX_F_* validity bitmask
};

// One entry in the GPU enumeration, for the in-app GPU picker. Mirrors `AdlxGpuInfo`
// in `amd.rs`.
struct AdlxGpuInfo {
    char     name[128];   // UTF-8, NUL-terminated
    int      type;        // ADLX_GPU_TYPE (0=undefined, 1=integrated, 2=discrete)
    unsigned vram_mb;     // total VRAM, MB
};

} // extern "C"

namespace {
// Kept alive across calls so the GPU/perf interfaces stay valid.
ADLXHelper g_help;
IADLXPerformanceMonitoringServicesPtr g_perf;
IADLXGPUPtr g_gpu;
bool g_ready = false;
} // namespace

extern "C" int adlx_init() {
    if (g_ready) return 0;

    ADLX_RESULT res = g_help.Initialize();
    if (!ADLX_SUCCEEDED(res)) return 1;

    IADLXSystem* sys = g_help.GetSystemServices();
    if (sys == nullptr) {
        g_help.Terminate();
        return 2;
    }

    res = sys->GetPerformanceMonitoringServices(&g_perf);
    if (!ADLX_SUCCEEDED(res)) {
        g_help.Terminate();
        return 3;
    }

    IADLXGPUListPtr gpus;
    res = sys->GetGPUs(&gpus);
    if (!ADLX_SUCCEEDED(res) || gpus->Empty()) {
        g_perf.Release();
        g_help.Terminate();
        return 4;
    }

    // Prefer the discrete GPU: on an APU + dGPU system (e.g. Ryzen iGPU + Radeon)
    // the first list entry is often the integrated GPU, whose tiny UMA VRAM and
    // idle metrics aren't what the gamer cares about. Fall back to the first GPU.
    IADLXGPUPtr first;
    for (auto it = gpus->Begin(); it != gpus->End(); ++it) {
        IADLXGPUPtr gpu;
        if (!ADLX_SUCCEEDED(gpus->At(it, &gpu)) || gpu == nullptr)
            continue;
        if (first == nullptr)
            first = gpu;
        ADLX_GPU_TYPE type = GPUTYPE_UNDEFINED;
        if (ADLX_SUCCEEDED(gpu->Type(&type)) && type == GPUTYPE_DISCRETE) {
            g_gpu = gpu;
            break;
        }
    }
    if (g_gpu == nullptr)
        g_gpu = first;
    if (g_gpu == nullptr) {
        g_perf.Release();
        g_help.Terminate();
        return 5;
    }

    g_ready = true;
    return 0;
}

extern "C" int adlx_sample(AdlxSample* out) {
    if (!g_ready || out == nullptr) return 1;

    *out = AdlxSample{};

    IADLXGPUMetricsPtr metrics;
    ADLX_RESULT res = g_perf->GetCurrentGPUMetrics(g_gpu, &metrics);
    if (!ADLX_SUCCEEDED(res) || metrics == nullptr) return 2;

    adlx_double d = 0.0;
    adlx_int    i = 0;

    if (ADLX_SUCCEEDED(metrics->GPUUsage(&d))) {
        out->gpu_usage = d;
        out->flags |= ADLX_F_USAGE;
    }
    if (ADLX_SUCCEEDED(metrics->GPUTemperature(&d))) {
        out->gpu_temp = d;
        out->flags |= ADLX_F_TEMP;
    }
    if (ADLX_SUCCEEDED(metrics->GPUHotspotTemperature(&d))) {
        out->gpu_hotspot = d;
        out->flags |= ADLX_F_HOTSPOT;
    }
    // Prefer total board power; fall back to GPU-only power.
    if (ADLX_SUCCEEDED(metrics->GPUTotalBoardPower(&d)) && d > 0.0) {
        out->gpu_power = d;
        out->flags |= ADLX_F_POWER;
    } else if (ADLX_SUCCEEDED(metrics->GPUPower(&d))) {
        out->gpu_power = d;
        out->flags |= ADLX_F_POWER;
    }
    if (ADLX_SUCCEEDED(metrics->GPUClockSpeed(&i))) {
        out->gpu_clock = i;
        out->flags |= ADLX_F_CLOCK;
    }
    if (ADLX_SUCCEEDED(metrics->GPUVRAMClockSpeed(&i))) {
        out->vram_clock = i;
        out->flags |= ADLX_F_VRAM_CLOCK;
    }
    if (ADLX_SUCCEEDED(metrics->GPUFanSpeed(&i))) {
        out->fan_rpm = i;
        out->flags |= ADLX_F_FAN;
    }
    if (ADLX_SUCCEEDED(metrics->GPUVRAM(&i))) {
        out->vram_used_mb = static_cast<unsigned>(i);
        out->flags |= ADLX_F_VRAM_USED;
    }

    adlx_uint total = 0;
    if (ADLX_SUCCEEDED(g_gpu->TotalVRAM(&total))) {
        out->vram_total_mb = total;
        out->flags |= ADLX_F_VRAM_TOTAL;
    }

    // System-wide FPS of the focused app (what AMD's own overlay shows). No PID
    // targeting, no admin — more robust than PresentMon for AMD users.
    IADLXFPSPtr fps;
    if (ADLX_SUCCEEDED(g_perf->GetCurrentFPS(&fps)) && fps != nullptr) {
        adlx_int f = 0;
        if (ADLX_SUCCEEDED(fps->FPS(&f)) && f > 0) {
            out->fps = f;
            out->flags |= ADLX_F_FPS;
        }
    }

    return 0;
}

// Number of GPUs ADLX sees, or -1 if not initialized. Indices passed to
// adlx_gpu_info / adlx_select are 0-based into this list.
extern "C" int adlx_gpu_count() {
    if (!g_ready) return -1;
    IADLXSystem* sys = g_help.GetSystemServices();
    if (sys == nullptr) return -1;
    IADLXGPUListPtr gpus;
    if (!ADLX_SUCCEEDED(sys->GetGPUs(&gpus)))
        return -1;
    return static_cast<int>(gpus->Size());
}

// Fill `out` with the name/type/VRAM of GPU `idx`. Returns 0 on success.
extern "C" int adlx_gpu_info(int idx, AdlxGpuInfo* out) {
    if (!g_ready || out == nullptr) return 1;
    *out = AdlxGpuInfo{};
    IADLXSystem* sys = g_help.GetSystemServices();
    if (sys == nullptr) return 2;
    IADLXGPUListPtr gpus;
    if (!ADLX_SUCCEEDED(sys->GetGPUs(&gpus))) return 3;
    IADLXGPUPtr gpu;
    if (!ADLX_SUCCEEDED(gpus->At(static_cast<adlx_uint>(idx), &gpu)) || gpu == nullptr)
        return 4;

    const char* name = nullptr;
    if (ADLX_SUCCEEDED(gpu->Name(&name)) && name != nullptr) {
        size_t i = 0;
        for (; name[i] != '\0' && i < sizeof(out->name) - 1; ++i)
            out->name[i] = name[i];
        out->name[i] = '\0';
    }
    ADLX_GPU_TYPE type = GPUTYPE_UNDEFINED;
    if (ADLX_SUCCEEDED(gpu->Type(&type)))
        out->type = static_cast<int>(type);
    adlx_uint vram = 0;
    if (ADLX_SUCCEEDED(gpu->TotalVRAM(&vram)))
        out->vram_mb = vram;
    return 0;
}

// Switch the GPU sampled by adlx_sample to index `idx`. Returns 0 on success.
extern "C" int adlx_select(int idx) {
    if (!g_ready) return 1;
    IADLXSystem* sys = g_help.GetSystemServices();
    if (sys == nullptr) return 2;
    IADLXGPUListPtr gpus;
    if (!ADLX_SUCCEEDED(sys->GetGPUs(&gpus))) return 3;
    IADLXGPUPtr gpu;
    if (!ADLX_SUCCEEDED(gpus->At(static_cast<adlx_uint>(idx), &gpu)) || gpu == nullptr)
        return 4;
    g_gpu = gpu;
    return 0;
}

extern "C" void adlx_shutdown() {
    if (!g_ready) return;
    g_gpu.Release();
    g_perf.Release();
    g_help.Terminate();
    g_ready = false;
}
