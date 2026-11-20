# ADR-003: Modelo de Licenciamiento y Procedencia de Contribuciones

**Fecha:** 2026-08-25
**Decidido:** 2026-08-25
**Estado:** Aceptado

## Contexto

El repositorio no contiene ningún archivo de licencia. En ausencia de licencia expresa, el código publicado se
considera bajo todos los derechos reservados en la mayoría de jurisdicciones: nadie puede usarlo, modificarlo ni
redistribuirlo, lo que contradice frontalmente el objetivo declarado de construir una comunidad de código
abierto.

Además, el proyecto declara dos ediciones (Community gratuita y Institution licenciada), y esa dualidad
condiciona qué licencias son viables y qué mecanismo de contribución se necesita.

Un factor adicional: la política de dependencias del proyecto prohíbe internamente las licencias recíprocas
fuertes por temor a que afecten a los conectores institucionales.

## Alternativas Consideradas

1. **Licencia permisiva para todo el núcleo, con separación arquitectónica de los componentes institucionales
   (núcleo abierto).** Adopción máxima, sin barreras para contribuir, sin necesidad de acuerdo de cesión. Un
   tercero puede competir con el código, pero no con la marca ni con la comunidad.
2. **Licencia recíproca fuerte con licencia dual comercial.** Todo el código abierto; quien no quiera cumplir la
   recíproca paga licencia comercial. Requiere obligatoriamente un acuerdo de licencia de contribuyente que
   permita relicenciar, lo que introduce fricción y excluye a parte de los contribuidores potenciales.
3. **Licencia recíproca fuerte sin licencia dual.** Máxima protección frente a apropiación, pero incompatible con
   vender una edición institucional con componentes cerrados.
4. **Sin publicar el código.** Descartada: contradice la identidad declarada del proyecto.

## Decisión

Se adopta la **alternativa 1**: licencia permisiva para el núcleo, con los componentes institucionales separados
arquitectónicamente, y declaración de origen de contribuciones mediante certificado de origen del desarrollador
en lugar de un acuerdo de cesión.

Elementos fijados:

| Elemento | Decisión |
|---|---|
| Licencia del núcleo | **Apache License 2.0** |
| Licencia de la documentación | Apache License 2.0, igual que el código, por simplicidad operativa |
| Procedencia de contribuciones | **DCO**: toda contribución lleva `Signed-off-by` (`git commit -s`) |
| Titularidad del copyright | Cada contribuidor conserva el suyo. El aviso colectivo es `Copyright 2026 The NB Books Authors` |
| Titularidad de la marca | Pendiente y **separada** de esta decisión; ver [trademark.md](../legal/trademark.md) |
| Frontera comercial | Separación arquitectónica: los servicios de la Suite Institucional son productos distintos del núcleo |

### Por qué Apache 2.0 y no MIT

Ambas son permisivas y ambas habilitan el modelo. Apache 2.0 aporta tres cosas que este proyecto concretamente
necesita:

1. **Concesión expresa de patentes, con terminación defensiva.** Un comprador institucional que evalúa riesgo
   prefiere una concesión explícita a un silencio. MIT no la contiene.
2. **Exclusión expresa de la marca.** La defensa real del proyecto no es la licencia, es el nombre. Apache 2.0
   declara que la licencia no concede derechos sobre nombres ni logotipos, lo que refuerza la
   [política de marca](../legal/trademark.md) en lugar de dejarla apoyada solo en el derecho de marcas.
3. **Obligación de conservar el archivo `NOTICE`.** Encaja con la obligación de atribución que el proyecto ya se
   impone en [third-party-notices.md](../legal/third-party-notices.md).

El coste es la incompatibilidad con dependencias GPLv2, ya que Apache 2.0 solo es compatible en un sentido con
GPLv3. Como la [política de dependencias](../architecture/dependency-policy.md) ya excluía esa
familia, el coste es nulo en la práctica.

### Por qué DCO y no acuerdo de licencia de contribuyente

Con licencia permisiva no hace falta poder relicenciar, que es la única razón real para exigir un acuerdo de
cesión. El DCO conserva el copyright de cada persona, no requiere firma ni gestión administrativa, y elimina una
barrera de entrada especialmente alta para contribuidores de instituciones públicas, cuyo departamento jurídico
tendría que revisar el acuerdo.

### Titularidad: por qué no bloquea

Con DCO no hay cesión, de modo que cada contribuidor conserva su copyright y el aviso correcto es colectivo
(`The NB Books Authors`). La pregunta sobre entidad jurídica sigue abierta, pero solo condiciona la **marca** y
la **capacidad de contratar**, no la licencia.

### Implementación

* `LICENSE` con el texto íntegro de Apache 2.0 en la raíz del repositorio.
* `NOTICE` con el aviso de copyright colectivo y la reserva de marca.
* `CONTRIBUTING.md` exige `git commit -s`; el CI verifica la presencia de `Signed-off-by`.

## Consecuencias

* **Positivas:** desbloquea la publicación pública; elimina la fricción para contribuidores individuales e
  institucionales; es coherente con la política de dependencias existente; deja abierta la venta institucional.
* **Negativas:** un tercero puede construir una oferta competidora sobre el mismo código. La defensa es la marca
  (ver [trademark.md](../legal/trademark.md)), la velocidad y la relación institucional, no la licencia.
* **Riesgos:** si en el futuro se quisiera cambiar a un modelo de licencia dual, el cambio requeriría el
  consentimiento individual de cada contribuidor. **Esta decisión es prácticamente irreversible.** Se toma de
  forma consciente y antes de aceptar la primera contribución externa, que es el único momento en que puede
  tomarse sin coste.
