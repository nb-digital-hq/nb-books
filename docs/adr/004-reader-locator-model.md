# ADR-004: Modelo de Posición y Anclaje de Anotaciones

**Fecha:** 2026-08-25
**Decidido:** 2026-08-25
**Estado:** Aceptado

## Contexto

El esquema actual almacena el progreso de lectura como un entero de 0 a 100. Ese modelo no sobrevive al contacto
con la realidad:

* Un EPUB reflowable no tiene páginas fijas; su posición depende del tamaño de fuente y de la ventana.
* Una anotación anclada a una posición numérica se desplaza cuando el usuario cambia la tipografía.
* La sincronización entre dispositivos con pantallas distintas produce posiciones incoherentes.
* La búsqueda semántica necesita devolver una posición exacta dentro del documento.

El proyecto declara longevidad de diez años. Perder las anotaciones de los usuarios por un modelo de posición
frágil es la forma más directa de incumplir esa promesa.

## Alternativas Consideradas

1. **Entero de progreso (actual).** Trivial, pero inservible para anotaciones y para formatos reflowables.
2. **Identificador de fragmento canónico del formato como única fuente.** Preciso cuando funciona, pero de
   soporte irregular entre implementaciones y frágil ante cambios del documento o cambios de edición.
3. **Modelo de Locator con múltiples selectores y degradación**, alineado con el modelo del ecosistema Readium y
   con el modelo de datos de anotación web: referencia al recurso, progresiones, identificador de fragmento
   cuando existe, y cita textual con contexto.
4. **Modelo propio no estándar.** Máxima libertad, máxima deuda: sin interoperabilidad ni exportación
   significativa.

## Decisión

Se adopta la **alternativa 3**. El progreso y las anotaciones se persisten como Locator. El anclaje de
anotaciones usa selectores en cascada: fragmento canónico, cita textual con contexto y posición aproximada. Una
anotación reanclada de forma aproximada se marca visiblemente como tal.

## Consecuencias

* **Positivas:** las anotaciones sobreviven a cambios de tipografía, de dispositivo y de edición; el modelo
  soporta de forma uniforme PDF, EPUB, cómic y audio; la exportación de anotaciones es interoperable; se habilita
  la reutilización futura de componentes del ecosistema Readium.
* **Negativas:** el modelo de datos es considerablemente más complejo que un entero; requiere migración del
  esquema actual y una implementación cuidadosa del reanclaje por cita.
* **Riesgos:** el reanclaje aproximado puede colocar una anotación en un lugar equivocado. La mitigación es no
  ocultarlo nunca: es preferible una anotación marcada como incierta que una anotación silenciosamente mal
  situada.
