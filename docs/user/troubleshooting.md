# Solución de problemas

## Un libro aparece como "no disponible"

El archivo ya no está donde estaba: puede que el disco esté desconectado o que hayas movido la carpeta. Conecta el
disco, o indica la nueva ubicación de la carpeta en Ajustes. Tus anotaciones y tu progreso no se pierden.

## La búsqueda no encuentra textos de algunos libros

El índice se construye en segundo plano. En Ajustes puedes ver su avance. Si se detuvo porque alcanzó el límite de
espacio, el motivo aparece ahí y puedes ampliar el límite.

## No hay sonido en la lectura en voz alta

* Para voces del sistema, comprueba que **Speech Dispatcher** esté instalado.
* Para la voz neuronal, instálala desde Ajustes.
* Con la AppImage o el `.tar.gz`, el sistema necesita los complementos de GStreamer `base` y `good`.

## La aplicación no abre

Comprueba que tu sistema tenga WebKitGTK 4.1. Con la AppImage en distribuciones recientes, prueba el paquete
`.tar.gz`, que usa las bibliotecas del sistema.

## Pedir ayuda

Abre un aviso en el repositorio del proyecto y adjunta el **paquete de diagnóstico** (Ajustes, Exportar
diagnósticos). Está redactado: no contiene rutas, títulos ni credenciales. Revísalo antes de enviarlo.
