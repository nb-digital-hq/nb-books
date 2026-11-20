<script lang="ts">
  // Las tipografías van antes que el resto: son el primer recurso que el
  // navegador necesita para maquetar, y con `font-display: block` cualquier
  // demora aquí es texto sin pintar.
  import '../fuentes.css';
  import '../app.css';

  import { leerCacheDeArranque } from '$core/settings';
  import { Notificaciones } from '$ui';
  import { usarIdioma } from '$lib/i18n';
  import { aplicarApariencia, elSistemaPrefiereOscuro, leerApariencia } from '$ui/tema';

  let { children } = $props();

  /*
   * La apariencia se aplica antes de pintar, desde la caché de arranque y no
   * desde el puente. Es la regla 2 de ADR-010: esperar a SQLite costaría un
   * destello de tema claro a quien usa tema oscuro, o de tipografía sin escalar
   * a quien depende de la escala. Cuando llegue el valor real, si difiere, gana
   * SQLite; eso ocurre al cargar las preferencias.
   */
  $effect.pre(() => {
    const arranque = leerCacheDeArranque();
    const raiz = document.documentElement;
    void aplicarApariencia(raiz, leerApariencia(arranque), elSistemaPrefiereOscuro());

    /*
     * El idioma, por el mismo camino y por el mismo motivo. `language` afecta al
     * primer pintado, así que sale de la caché sin esperar al puente. Su
     * diccionario ya está en memoria, porque lo carga `+layout.ts` antes de montar
     * nada: aquí el cambio es inmediato y el primer fotograma sale en su idioma.
     *
     * También al elemento raíz: sin `lang` correcto, el lector de pantalla lee
     * el texto con la pronunciación del idioma equivocado.
     */
    void usarIdioma(arranque.language);
    raiz.lang = typeof arranque.language === 'string' ? arranque.language : 'es';
  });
</script>

{@render children()}

<!--
  Los avisos viven aquí y no dentro de la página: en lectura inmersiva la
  carcasa entera se retira, y un avisador montado dentro de ella se iría con
  ella justo cuando alguien acaba de anotar sin verlo.
-->
<Notificaciones />
