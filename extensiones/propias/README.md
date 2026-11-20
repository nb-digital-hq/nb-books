# Extensiones de primera parte

Las que publica el proyecto. Entran por el **mismo camino** que las de terceros —manifiesto, permisos,
revisión automatizada y firma—, y ninguna recibe trato especial: un privilegio de primera parte es exactamente
el atajo que deja la API sin probar.

| Directorio | Qué aporta |
|---|---|
| `formato-texto/` | Proveedor de formato de texto plano estructurado con secciones |
| `ordenes/` | Aporta órdenes ejecutables para la paleta y atajos |
| `vista-resumen/` | Aporta vistas de estudio estructuradas con bloques tipados |
| `metadatos-academicos/` | Una fuente de metadatos que consulta Crossref **por DOI** |
| `personalizacion/` | Un tema y tres modos de lectura. **No trae código**: es un manifiesto y nada más |
| `normalizador/` | Corrige metadatos por lotes. **Propone y nunca escribe** |
| `citas/` | Exportación de citas a BibTeX y RIS |
| `fondo-sonoro/` | Tres fondos sonoros para concentrarse. **Ni un byte de audio**: declara qué clase de ruido y lo sintetiza el núcleo |

## Por qué van aparte del banco

`extensiones/banco/` son módulos adversos: existen para que el cajón de arena tenga contra qué probarse, e
intentan a propósito lo que no deben. Esto son extensiones que se publican. Mezclarlos habría hecho que
construir el banco construyera lo que se distribuye, y al revés.

## Cómo se construyen

```bash
cd extensiones/propias
cargo build --target wasm32-wasip2 --target-dir target
```

Se compilan a **componentes** (`wasm32-wasip2`), que es el camino por el que entran las extensiones de verdad.
El objetivo se instala una vez con `rustup target add wasm32-wasip2`; sin él, las pruebas que las usan se saltan
solas en lugar de fallar.

El `.wasm` **no se comita**: lo impiden `.gitignore` y el gancho de pre-commit, igual que con el banco.

## Cómo se instalan en la aplicación

**No desde estas carpetas.** Cada una son las fuentes de la extensión: el manifiesto y el código, sin el `.wasm`
que el manifiesto declara. El 2026-09-12 se instalaron así, entraron sin queja y cada uso respondía "no se
encontró el archivo". Desde esta parte la aplicación rechaza un paquete al que le falta un módulo, y lo que hay que
instalar es el paquete construido:

```bash
pnpm extensiones:empaquetar
```

Compila en release y deja cada extensión, completa, en `extensiones/propias/dist/<nombre>/`. Esa es la carpeta
que se elige en Ajustes, Extensiones, Instalar. `dist/` no entra al repositorio.

## Cómo se revisa y se firma una antes de publicarla

```bash
cargo run --features herramientas --bin revisar -- extensiones/propias/metadatos-academicos --de-primera-parte
cargo run --features herramientas --bin firmar  -- extensiones/propias/metadatos-academicos --clave <la clave del proyecto>
```

`--de-primera-parte` es lo único que las distingue en la revisión, y es lo que autoriza el espacio de nombres
`lat.nbdigitallabs.books.*` (requisito R2 de la política). **La clave de firma no vive en el repositorio** y no va a vivir:
quien la tenga puede publicar actualizaciones de todas las extensiones del proyecto.
