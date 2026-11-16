# Licenciamiento

> **Estado:** Vigente. Ver [ADR-003](../adr/003-licensing-model.md).
> **Aviso:** este documento recoge criterios de ingeniería y gestión de riesgo. No constituye asesoría legal.

## 1. Decisión vigente

| Elemento | Decisión |
|---|---|
| Licencia del núcleo | **Apache License 2.0** |
| Licencia de la documentación | Apache License 2.0 |
| Modelo comercial | **Núcleo abierto**, con los componentes institucionales separados arquitectónicamente |
| Procedencia de contribuciones | **DCO** (`Signed-off-by`), sin acuerdo de cesión |
| Aviso de copyright | `Copyright 2026 The NB Books Authors`; cada contribuidor conserva el suyo |
| Marca | "NB", "NB Digital" y "NB Books", excluidas de la licencia; ver [trademark.md](trademark.md) |

Los archivos `LICENSE` y `NOTICE` están en la raíz del repositorio.

## 2. Qué significa en la práctica

### Para quien usa NB Books
Puede usarlo, modificarlo, redistribuirlo e incorporarlo en productos propios, incluidos productos comerciales
cerrados, siempre que conserve el aviso de copyright, el texto de la licencia, el archivo `NOTICE` y la
indicación de los cambios realizados. **No** obtiene derecho a usar el nombre ni el logotipo de NB Books; eso lo
rige la [política de marca](trademark.md).

### Para quien contribuye
Conserva el copyright de su aportación y la licencia bajo Apache 2.0. No firma ningún acuerdo. Basta con
declarar el origen de su trabajo mediante `git commit -s`, que añade la línea `Signed-off-by`. Esa línea es una
afirmación de que la persona tiene derecho a aportar ese código bajo la licencia del proyecto.

### Para una institución que evalúa la adopción
Apache 2.0 es una licencia aprobada por la Open Source Initiative, ampliamente aceptada en contratación pública y
privada, con concesión expresa de patentes. La edición institucional se licencia por separado; el núcleo que la
institución instala en los equipos es Apache 2.0.

## 3. La frontera del núcleo abierto

La separación entre lo abierto y lo comercial no es una opción de compilación: es una frontera arquitectónica.

| Componente | Licencia |
|---|---|
| Aplicación de escritorio, motor de lectura, biblioteca, búsqueda, IA local, sistema de extensiones | Apache 2.0 |
| Productos de la Suite Institucional de NB Digital, que se distribuyen por separado | Licencia comercial |
| Documentación operativa, soporte, despliegue e integración | Servicio comercial |

Si esa frontera se difumina, la licencia deja de ser aplicable en la práctica. Cualquier PR que introduzca en el
núcleo abierto una dependencia funcional hacia un producto comercial debe rechazarse.

## 4. Riesgo asumido conscientemente

Con licencia permisiva, un tercero puede tomar el código y construir una oferta competidora, incluida una oferta
institucional. Se acepta. La defensa del proyecto no es la licencia, son tres cosas distintas:

1. **La marca.** Un fork no puede llamarse NB Books.
2. **La velocidad y el conocimiento del dominio.** Mantener el proyecto es más caro que copiarlo una vez.
3. **La relación institucional.** Lo que una universidad compra no es el binario, es el soporte, la integración
   y la responsabilidad contractual.

## 5. Consecuencias operativas

* **Dependencias:** la licencia del proyecto restringe qué dependencias puede incorporar. La tabla vigente está
  en la [política de dependencias](../architecture/dependency-policy.md). En particular, GPLv2 pasa
  de "requiere escalamiento" a **prohibida**, porque es incompatible con Apache 2.0.
* **Atribución:** obligación de conservar `NOTICE`; ver [third-party-notices.md](third-party-notices.md).
* **Extensiones:** el registro exige que cada extensión declare su propia licencia. No tienen que ser Apache 2.0,
  pero sí compatible con su redistribución. Ver
  [governance/extension-policy.md](governance/extension-policy.md).
* **Modelos de IA:** los pesos tienen licencia propia y algunas restringen el uso comercial. Se revisan por
  separado.

## 6. Irreversibilidad

Cambiar a un modelo de licencia dual en el futuro exigiría el consentimiento individual de cada persona que haya
contribuido. **Esta decisión se tomó de forma consciente antes de aceptar la primera contribución externa**, que
era el único momento en que podía tomarse sin coste.
