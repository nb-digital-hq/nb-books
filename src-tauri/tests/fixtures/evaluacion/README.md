# Corpus de evaluación de las funciones de estudio

Contrato: [docs/architecture/ai/evaluation.md](../../../../docs/architecture/ai/evaluation.md) sección 2. Adelantado
de **F4.4.1** a F4.3.4 por decisión de producto el 2026-09-17, para que "supera la rúbrica" tenga contra qué medirse.

> **Procedencia:** todos los textos están **escritos para el proyecto** el 2026-09-17. Personas, lugares, fechas y
> cifras son inventados salvo donde se dice. Ninguno contiene material con derechos de terceros.
>
> **Licencia:** Apache-2.0, igual que el resto del repositorio.

| Archivo | Idioma | Clase | Qué pone a prueba |
|---|---|---|---|
| `tecnico-es.txt` | Español | Técnico, con cifras | Que las tarjetas pregunten por relaciones de causa y por cifras que están en el texto |
| `narrativo-en.txt` | Inglés | Narrativo | Que el resumen conserve el orden de los hechos y no invente un desenlace |
| `academico-pt.txt` | Portugués | Académico, con una cita (Silva e Moreira, 2019, **inventada**) | Que una cita del texto no se convierta en una afirmación del modelo |
| `ocr-es.txt` | Español | Estructura pobre: guiones de fin de línea, tildes perdidas y mayúsculas de OCR | Que un texto roto no produzca preguntas sobre palabras partidas |
| `../adversario-prompt.txt` | Español | Adversario | Ver F4.3.2 |

Faltan, y están dichos en `evaluation.md`: un texto con fórmulas y tablas, y uno narrativo **largo**. Se añaden con
F4.4.1, que sigue abierta.

## Hechos de referencia

Cada texto tiene en `referencias.json` los hechos que una buena tarjeta o un buen resumen tienen que poder sostener.
**Los escribió quien escribió el texto, no una persona distinta**: `evaluation.md` pide salidas de referencia
revisadas por una persona, y esa revisión es humana.
