// CPU temperature sidecar for Meteor's metrics overlay.
//
// LibreHardwareMonitor reads the CPU package temperature (Ryzen Tctl/Tdie, Intel
// core) via a kernel driver it loads at runtime — so this must run elevated; if
// the driver can't load, no temperature sensor appears and we print nothing, and
// the Rust side simply omits CPU temp (best-effort, like PresentMon).
//
// Protocol: one integer (°C) per line on stdout, ~once per second. That's all the
// Rust controller (cputemp.rs) parses.
//
// Shutdown protocol: the parent closes our stdin, we see EOF, call computer.Close()
// and exit. This matters more than it looks — Open() makes LibreHardwareMonitor
// install and start a kernel driver via the SCM, and only Close() unloads it. Being
// TerminateProcess'd (which is what a kill or the parent's kill-on-close Job Object
// does) skips .NET finalizers, so the driver would stay loaded and registered for
// the rest of the boot. That residue is what vulnerable-driver blocklists and kernel
// anti-cheats look for, so it must not depend on the happy path alone.

using System.Globalization;
using LibreHardwareMonitor.Hardware;

var computer = new Computer { IsCpuEnabled = true };
try
{
    computer.Open();
}
catch (Exception e)
{
    Console.Error.WriteLine("cputemp: open failed: " + e.Message);
    return 1;
}

// Unload the driver exactly once, whichever path we leave by.
var closed = 0;
void Shutdown()
{
    if (Interlocked.Exchange(ref closed, 1) != 0) return;
    try { computer.Close(); }
    catch (Exception e) { Console.Error.WriteLine("cputemp: close failed: " + e.Message); }
}

// Covers a normal return and an unhandled exception; TerminateProcess still cannot
// be intercepted, which is why the parent asks over stdin instead of killing.
AppDomain.CurrentDomain.ProcessExit += (_, _) => Shutdown();

using var stop = new ManualResetEventSlim(false);

Console.CancelKeyPress += (_, e) =>
{
    // Handle it ourselves so the sampling loop can unwind through Shutdown().
    e.Cancel = true;
    stop.Set();
};

// EOF on stdin = the parent dropped its write handle and wants us gone.
new Thread(() =>
{
    try { Console.In.ReadToEnd(); }
    catch { /* closed underneath us; treat as a stop request */ }
    stop.Set();
})
{ IsBackground = true, Name = "stdin-watch" }.Start();

var visitor = new UpdateVisitor();

while (!stop.IsSet)
{
    computer.Accept(visitor);

    float? temp = null;
    foreach (IHardware hw in computer.Hardware)
    {
        if (hw.HardwareType != HardwareType.Cpu) continue;

        // Prefer a package/Tctl/Tdie sensor; fall back to the hottest core.
        float? pkg = null;
        float? maxCore = null;
        foreach (ISensor s in hw.Sensors)
        {
            if (s.SensorType != SensorType.Temperature || s.Value is not float v) continue;
            string name = s.Name ?? string.Empty;
            if (name.Contains("Package") || name.Contains("Tctl") || name.Contains("Tdie"))
                pkg = v;
            else if (name.Contains("Core"))
                maxCore = maxCore is float m ? Math.Max(m, v) : v;
        }
        temp = pkg ?? maxCore;
        if (temp is not null) break;
    }

    if (temp is float t)
    {
        Console.WriteLine(((int)Math.Round(t)).ToString(CultureInfo.InvariantCulture));
        Console.Out.Flush();
    }

    // Wait, but wake immediately when the parent asks us to stop, so a shutdown
    // never has to sit through the rest of a sampling second.
    if (stop.Wait(1000)) break;
}

Shutdown();
return 0;

// Walks the hardware tree and calls Update() so sensor values refresh.
sealed class UpdateVisitor : IVisitor
{
    public void VisitComputer(IComputer computer) => computer.Traverse(this);
    public void VisitHardware(IHardware hardware)
    {
        hardware.Update();
        foreach (IHardware sub in hardware.SubHardware) sub.Accept(this);
    }
    public void VisitSensor(ISensor sensor) { }
    public void VisitParameter(IParameter parameter) { }
}
