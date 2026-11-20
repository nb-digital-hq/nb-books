# Cumplimiento de Accesibilidad

> **Estado:** Vigente.
> Complementa [architecture/accessibility.md](../architecture/accessibility.md), que cubre el
> **cómo** técnico. Este documento cubre el **qué exige la norma** y por qué es un requisito, no una mejora.

## 1. La accesibilidad no es opcional en este producto

NB Books es una aplicación de lectura dirigida a instituciones educativas. Esto lo sitúa en el centro de la
normativa de accesibilidad digital por tres vías simultáneas:

1. Es una aplicación de lectura de libros electrónicos.
2. Se vende a instituciones públicas y privadas de educación, que suelen tener obligaciones de accesibilidad en
   su contratación.
3. Sirve a personas con discapacidad visual, para quienes un lector accesible es la diferencia entre acceder o no
   al material de estudio.

En la Unión Europea, la normativa de accesibilidad aplicable desde 2025 alcanza expresamente a los libros
electrónicos, a los programas de lectura y a los sistemas de gestión de derechos digitales asociados. La
referencia técnica habitual es el nivel AA de las pautas de accesibilidad para el contenido web, junto con el
estándar específico de accesibilidad para publicaciones digitales.

Consecuencia práctica: **una institución europea puede quedar impedida de contratar NB Books si el producto no
cumple.** Y una institución de cualquier lugar puede exigirlo en su pliego.

## 2. Estándar de referencia adoptado

El proyecto adopta como objetivo el **nivel AA de las pautas de accesibilidad para el contenido web en su
versión vigente**, aplicado tanto a la interfaz de la aplicación como a la experiencia de lectura.

Adoptar la versión más reciente en lugar del mínimo exigido protege frente a la actualización de las referencias
normativas, que ocurre con regularidad.

## 3. Alcance: qué debe ser accesible

| Componente | Requisito |
|---|---|
| Carcasa de la aplicación (barra, navegación, área principal, paneles) | Navegación completa por teclado, puntos de referencia expuestos al lector de pantalla, foco visible |
| Biblioteca | Cuadrícula navegable, alternativas textuales para portadas, búsqueda accesible |
| Lector de EPUB | Preservación de la semántica del documento, control de tipografía y espaciado, respeto de las preferencias del usuario |
| Lector de PDF | Exposición de la capa de texto; declaración explícita cuando el documento no tiene texto accesible |
| Anotaciones | Creación, consulta y edición sin ratón |
| Configuración | Alto contraste, escala tipográfica, movimiento reducido, lectura en voz alta |
| Documentación | La propia documentación debe ser accesible |

## 4. Punto crítico: PDF renderizado como imagen

La arquitectura actual renderiza páginas de PDF como imágenes. Sin medidas adicionales, esas páginas son
completamente opacas para un lector de pantalla, lo que convierte la función principal del producto en
inaccesible.

La mitigación (superponer la capa de texto extraída) no es una mejora futura: es un requisito de cumplimiento y
debe estar en el alcance de la primera versión que se ofrezca a una institución. Ver
[reader-contracts.md](../architecture/reader-contracts.md).

## 5. Verificación

* Comprobación automatizada en cada PR sobre el shell y las vistas principales.
* Verificación manual por release con los lectores de pantalla de referencia de cada plataforma.
* Pruebas con usuarios reales de tecnologías de asistencia antes de la primera versión estable. Ninguna
  herramienta automatizada sustituye esto.

## 6. Declaración de accesibilidad

El proyecto debe publicar una declaración de accesibilidad que indique el estándar aplicado, el grado de
conformidad, las limitaciones conocidas y el canal para reportar barreras. Publicar las limitaciones conocidas no
debilita la posición del proyecto: la ausencia de declaración sí.
