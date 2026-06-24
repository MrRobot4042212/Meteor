// CPU temperature sidecar for Meteor's metrics overlay.
//
// LibreHardwareMonitor reads the CPU package temperature (Ryzen Tctl/Tdie, Intel
// core) via a kernel driver it loads at runtime — so this must run elevated; if
// the driver can't load, no temperature sensor appears and we print nothing, and
// the Rust side simply omits CPU temp (best-effort, like PresentMon).
//
// Protocol: one integer (°C) per line on stdout, ~once per second. That's all the
// Rust controller (cputemp.rs) parses.

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

var visitor = new UpdateVisitor();

while (true)
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

    Thread.Sleep(1000);
}

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
