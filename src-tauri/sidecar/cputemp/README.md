# cputemp — sidecar de temperatura de CPU

Pequeño ejecutable .NET que transmite la **temperatura de la CPU** (°C, un entero
por línea a stdout, ~1/s) para el overlay de métricas. Lo lanza y lo parsea
`src-tauri/src/cputemp.rs`.

## Por qué un sidecar

La temperatura real de la CPU (Ryzen **Tctl/Tdie**, núcleos Intel) en Windows solo
se lee con un **driver de kernel**. Aquí lo provee
[LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor)
(`LibreHardwareMonitorLib`), que carga su driver en runtime. Implicaciones:

- **Requiere ejecutar Meteor como administrador** (cargar el driver). Sin admin, el
  sensor no aparece y `cputemp.rs` deja la temperatura en `None` (best-effort, igual
  que PresentMon).
- **Windows 11 con Memory Integrity (HVCI)** bloquea el WinRing0 clásico. Usa una
  versión de `LibreHardwareMonitorLib` con el **driver compatible con HVCI** (las
  series `0.9.7-pre*` en adelante; las estables viejas como `0.9.4` quedan bloqueadas
  y la temperatura se lee como 0).

## Compilar

```bash
cd src-tauri/sidecar/cputemp
# Self-contained single-file → src-tauri/binaries/cputemp.exe (no necesita .NET en destino)
dotnet publish -c Release -r win-x64 --self-contained true \
  -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true \
  -p:DebugType=none -o ../../binaries
```

- Para **desarrollo** (`npm run app`) con esto basta: `cputemp.rs` busca el exe en
  `binaries/cputemp.exe` (cwd = `src-tauri`), junto al ejecutable y en `resource_dir`.
- El binario **no se versiona** (vive en `binaries/`, como `PresentMon.exe`).
- Para el **instalador** (`npm run app:build`), empaquétalo añadiendo a
  `src-tauri/tauri.conf.json` dentro de `"bundle"`:
  ```json
  "resources": ["binaries/cputemp.exe"]
  ```
  (No lo añadas hasta tener el `.exe` aquí, o el build de release fallará. El workflow
  de release debe construir este sidecar antes de empaquetar.)

## Diagnóstico

Ejecutarlo a mano (como admin) imprime la temperatura cada segundo. Si imprime `0` o
nada, el driver no cargó: revisa elevación y HVCI/blocklist de drivers.
