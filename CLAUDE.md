# CLAUDE.md — Contrato del proyecto Nexo

Contexto para Claude Code al ampliar este proyecto.

## Qué es

Launcher de escritorio para Windows que unifica juegos y apps en una sola
biblioteca. Tauri 2 (Rust) + Next.js 16 con **export estático** (`output: 'export'`).
No hay backend de red: la lógica nativa son comandos Rust invocados con
`@tauri-apps/api/core` → `invoke`.

## Reglas de arquitectura (respetar)

- **Toda lógica de sistema va en Rust**, en módulos por responsabilidad
  (`steam.rs`, `launcher.rs`, `storage.rs`, …). El front nunca toca el disco ni
  procesos directamente: siempre a través de un comando Tauri.
- **Cada fuente nueva = su propio módulo** (p. ej. `epic.rs`, `gog.rs`) que
  devuelve `Vec<Game>` y se mezcla en `get_library`. No meter parsing de varias
  tiendas en el mismo archivo.
- El modelo común es `Game` (`models.rs`). Si una fuente necesita campos nuevos,
  añadirlos como `Option<…>` con `#[serde(default)]` para no romper datos guardados.
- Los comandos devuelven `Result<T, String>`; el front muestra el error, nunca
  hace `panic`. Un fallo de una fuente no debe tumbar el resto de la biblioteca.
- Frontend tipado: todo comando tiene su wrapper en `src/lib/tauri.ts` y su tipo
  en `src/lib/types.ts` espejando el `struct` de Rust.
- Permisos: cualquier plugin nuevo de Tauri se declara en
  `src-tauri/capabilities/default.json`.

## Limitaciones a tener presentes

- Sin SSR ni API routes (export estático). Nada de `fetch` a rutas internas.
- Las imágenes externas funcionan porque `csp` está en `null` en `tauri.conf.json`.
  Si se endurece la CSP, hay que permitir explícitamente el CDN de Steam.

## Roadmap (siguientes fases)

1. **Más fuentes**: Epic (manifiestos `.item` en `%PROGRAMDATA%\Epic\...`),
   GOG (registro), apps instaladas de Windows (claves de desinstalación del
   registro + accesos directos del menú inicio).
2. **Metadatos y arte**: integrar SteamGridDB (requiere API key del usuario) e
   IGDB para portadas/géneros de apps que no son de Steam.
3. **Calidad de vida**: favoritos y "jugados recientemente" (persistidos junto a
   los manuales), categorías/etiquetas, ordenación.
4. **Tiempo de juego**: vigilar el proceso lanzado y acumular horas por juego.
5. **Opcional en la nube**: backend Nest.js para sincronizar biblioteca y ajustes
   entre varios PCs (aquí sí entraría Nest, no antes).

## Comandos

```bash
npm install
npm run app         # desarrollo (tauri dev)
npm run app:build   # instalador
```
