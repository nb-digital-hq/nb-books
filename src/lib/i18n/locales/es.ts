/**
 * El diccionario base: español.
 *
 * Es la **fuente**, no una traducción. Su forma define el tipo `Diccionario`, y
 * por eso los otros idiomas no pueden olvidarse una clave: el compilador se lo
 * impide. Añadir un texto aquí obliga a añadirlo en los tres.
 *
 * # Cómo se organiza
 *
 * Un espacio de nombres por componente o por vista, con el mismo nombre que
 * usa el componente. No por tema ni por tipo de texto: cuando alguien cambia
 * una pantalla quiere ver junto todo lo que esa pantalla dice, y un diccionario
 * ordenado por "botones", "errores" y "títulos" obliga a recorrerlo entero.
 *
 * # Qué no está aquí
 *
 * Los mensajes de error del runtime van en `errores`, y los alimenta el
 * catálogo de códigos canónicos de Rust: ver `traducirError` en `errores.ts`.
 * Rust nunca manda texto traducido (`i18n.md`, sección 3).
 *
 * Contrato: `docs/architecture/i18n.md`.
 */

export const es = {
  carcasa: {
    plegarTarjeta: 'Plegar la Tarjeta',
    desplegarTarjeta: 'Desplegar la Tarjeta',
    buscarAyuda: 'Busca obras, autores o una frase',
    borrarBusqueda: 'Borrar la Búsqueda',
    colecciones: 'Colecciones',
    importarCorto: 'Importar',
    audiolibrosCorto: 'Audiolibros',
    audiolibros: 'Preparar Audiolibros',
    leyendo: 'Leyendo',
    tarjetaIndice: 'Índice de Búsqueda',
    tarjetaIndiceDe: 'de',
    tarjetaIndiceObras: 'obras',
    tarjetaIndiceSinSitio: 'Sin sitio',
    tarjetaIndicePausado: 'En pausa',
    tarjetaImportacion: 'Importando',
    tarjetaImportacionDe: 'de',
    tarjetaImportacionDocumentos: 'documentos',
    tarjetaEstado: 'Algo necesita atención',
    tarjetaEstadoAyuda: 'Míralo en Acerca de',
    navegacion: 'Navegación Principal',
    salto: 'Saltar al Contenido',
    inmersion: 'Lectura inmersiva. Pulsa Escape para volver.',
    cerrarPanel: 'Cerrar panel',
    plegar: 'Plegar la Barra',
    desplegar: 'Desplegar la Barra',
    buscar: 'Buscar',
    importar: 'Importar Documentos',
    estado: 'Estado del Runtime',
    cerrarDocumento: 'Cerrar el Documento',
    version: 'Versión',
    esquema: 'Esquema',
    recuperada: 'No se pudo abrir tu biblioteca',
    recuperadaAyuda:
      'La aplicación ha arrancado con una biblioteca vacía. La anterior no se ha borrado: está en tu carpeta de datos con este nombre.',
    recuperadaMotivo: 'Motivo',
    sinIteracion: 'Esta sección llega con su iteración.'
  },

  destinos: {
    inicio: 'Inicio',
    biblioteca: 'Biblioteca',
    descubrir: 'Descubrir',
    lector: 'Lector',
    estudio: 'Estudio',
    ajustes: 'Ajustes'
  },

  paneles: {
    colecciones: 'Colecciones',
    indice: 'Índice',
    anotaciones: 'Anotaciones',
    consulta: 'Consulta',
    voz: 'Lectura en Voz Alta',
    modoDeLectura: 'Modo de Lectura',
    ajustesDelLibro: 'Ajustes del Libro',
    ficha: 'Ficha',
    busqueda: 'Búsqueda',
    estudio: 'Estudiar con IA',
    importar: 'Importar',
    audiolibros: 'Audiolibros',
    estado: 'Estado del Runtime'
  },

  inicio: {
    region: 'Inicio',
    retomar: 'Sigue Leyendo',
    continuar: 'Continuar',
    detalles: 'Detalles',
    leido: 'leído',
    seguirLeyendo: 'Seguir Leyendo',
    empezados: 'empezados',
    audiolibros: 'Audiolibro con IA',
    novedades: 'Novedades de la Biblioteca',
    materias: 'Explorar por Materia',
    obras: 'obras',
    verTodo: 'Ver Todo',
    anteriores: 'Anteriores',
    siguientes: 'Siguientes',
    minutosHoy: 'min hoy',
    minutosSemana: 'min esta semana',
    racha: 'días seguidos',
    rachaUno: 'día seguido',
    vacio: 'Tu biblioteca está vacía. Importa documentos para empezar.',
    sinEmpezar: 'Todavía no has empezado ningún documento.',
    importar: 'Importar Documentos',
    irABiblioteca: 'Ver la Biblioteca',
    masDe: 'Más de',
    redescubrir: 'Aún sin Abrir',
    alAzar: 'Para Hoy, al Azar',
    porqueNovedades: 'Lo último que entró',
    porqueLees: 'Porque estás leyendo',
    porqueMateria: 'Una materia de tu biblioteca',
    porqueAutor: 'De los autores con más obras en tu biblioteca',
    porqueColeccion: 'Tu colección',
    porqueRedescubrir: 'Obras que nunca has abierto, distintas en cada visita',
    porqueAlAzar: 'Diez obras distintas en cada visita'
  },

  biblioteca: {
    anclas: 'Saltar a una letra',
    ascendente: 'De Menor a Mayor',
    descendente: 'De Mayor a Menor',
    irAPagina: 'Ir a la Página',
    region: 'Biblioteca',
    controles: 'Controles de la Biblioteca',
    ordenar: 'Ordenar por',
    filtrar: 'Filtrar',
    estanterias: 'Estanterías',
    vista: 'Forma de la Vista',
    cuadricula: 'Ver en Cuadrícula',
    lista: 'Ver en Lista',
    mosaico: 'Ver en Mosaico',
    cargando: 'Cargando',
    porFecha: 'Añadido',
    porTitulo: 'Título',
    porAutor: 'Autor',
    porSerie: 'Serie',
    porProgreso: 'Progreso',
    columnaMarca: 'Estado',
    columnaFormato: 'Formato',
    columnaTamano: 'Tamaño',
    porLectura: 'Leído por Última Vez',
    porPublicacion: 'Publicación',
    todos: 'Todos',
    empezados: 'Empezados',
    duplicados: 'Duplicados',
    duplicadosCriterio: 'con el mismo título y autor',
    noDisponibles: 'No disponibles',
    vacio: 'Todavía no hay documentos. Importa uno para empezar.',
    sinResultados: 'Ningún documento cumple este filtro.',
    error: 'No se pudo leer la biblioteca.',
    reintentar: 'Reintentar',
    noDisponible: 'No disponible',
    copias: 'copias',
    documentos: 'documentos',
    unDocumento: 'documento',
    paginacion: 'Páginas de la Biblioteca',
    revisarMetadatos: 'Revisar Metadatos',
    porPagina: 'Por página',
    deTotal: 'de',
    anterior: 'Página Anterior',
    siguiente: 'Página Siguiente'
  },

  estanterias: {
    titulo: 'Estanterías',
    buscar: 'Buscar una Estantería',
    buscarAyuda: 'Autor, serie o materia',
    quitar: 'Ver toda la biblioteca',
    cargando: 'Buscando',
    vacio: 'Esta biblioteca todavía no tiene autores, series ni materias.',
    sinResultados: 'Ninguna estantería coincide.',
    deTotal: 'de',
    'clase-author': 'Autores',
    'clase-series': 'Series',
    'clase-subject': 'Materias',
    'clase-language': 'Idiomas',
    'clase-format': 'Formatos',
    'clase-year': 'Años',
    'formato-application/epub+zip': 'EPUB',
    'formato-application/pdf': 'PDF',
    'formato-application/vnd.comicbook+zip': 'Cómic'
  },

  colecciones: {
    titulo: 'Colecciones',
    nueva: 'Nueva Colección',
    nombre: 'Nombre de la Colección',
    crear: 'Crear',
    cancelar: 'Cancelar',
    renombrar: 'Cambiar el Nombre',
    guardar: 'Guardar',
    borrar: 'Borrar',
    borrarPregunta: '¿Borrar esta colección? Sus libros se quedan en la biblioteca.',
    vacio: 'Todavía no tienes colecciones. Crea una y añade libros desde su ficha.',
    repetido: 'Ya tienes una colección con ese nombre.',
    sinNombre: 'Ponle un nombre.',
    verToda: 'Ver toda la biblioteca',
    cargando: 'Cargando'
  },

  teclas: {
    ArrowUp: 'Flecha Arriba',
    ArrowDown: 'Flecha Abajo',
    ArrowLeft: 'Flecha Izquierda',
    ArrowRight: 'Flecha Derecha',
    Escape: 'Esc',
    Enter: 'Intro',
    Control: 'Ctrl',
    Shift: 'Mayús'
  },

  ficha: {
    autorizar: 'Autorizar',
    autoresMas: 'y {n} más',
    editorial: 'Editorial',
    publicado: 'Publicado',
    idioma: 'Idioma',
    materias: 'Materias',
    archivo: 'Archivo',
    colecciones: 'Colecciones',
    coleccionesVacio: 'Crea una colección desde la barra superior y podrás añadir aquí esta obra.',
    vacio: 'Elige una obra para ver su ficha.',
    leer: 'Leer',
    continuar: 'Continuar',
    progreso: 'Progreso',
    sinEmpezar: 'Sin empezar',
    terminado: 'Terminado',
    serie: 'Serie',
    anadido: 'Añadido',
    copias: 'Copias',
    copiasValor: 'en la biblioteca',
    completar: 'Completar Desde un Catálogo',
    completarAyuda:
      'Sale a un catálogo abierto con el título y el autor; lo que llegue no pisa lo tuyo.',
    ordenesDeExtension: 'Lo que aportan tus extensiones',
    ordenesDeExtensionAyuda:
      'Estas acciones las añaden extensiones instaladas. Su código no se carga hasta que pulsas una.',
    buscando: 'Buscando',
    propuestas: 'Propuestas Sin Aplicar',
    aceptar: 'Aceptar',
    rechazar: 'Rechazar',
    'campo-title': 'Título',
    'campo-publisher': 'Editorial',
    'campo-published_date': 'Publicación',
    'campo-language': 'Idioma',
    'error-NETWORK_DISABLED': 'Está puesto el modo sin red. Se apaga en Ajustes.',
    'error-CONSENT_REQUIRED':
      'Falta autorizar la salida a Open Library, el catálogo abierto del que salen los datos.',
    'error-NETWORK_FAILURE': 'El catálogo no respondió.',
    noDisponible: 'El archivo no está donde debería',
    noDisponibleAyuda: 'Conecta el volumen o vuelve a importarlo. Sus anotaciones siguen aquí.'
  },

  lector: {
    region: 'Lector',
    controles: 'Acciones del documento',
    controlesDeLoElegido: 'Acciones sobre lo elegido',
    nota: 'Nota',
    citando: 'Sobre',
    guardar: 'Guardar',
    cancelar: 'Cancelar',
    capitulo: 'Capítulo',
    de: 'de',
    pagina: 'Página',
    progreso: 'Progreso de la lectura',
    segundaMitad: 'Elige la segunda obra para leerlas a la vez.',
    aproximado: 'La posición se ha aproximado y puede no ser exacta',
    error: 'No se pudo completar la acción'
  },

  superficie: {
    region: 'Documento',
    pagina: 'Página',
    sinCapaDeTexto: 'Este documento no tiene texto: es un escaneado sin OCR.',
    cargando: 'Cargando el documento'
  },

  indice: {
    vacio: 'Este documento no trae índice.',
    aqui: 'Estás aquí'
  },

  anotaciones: {
    lista: 'Anotaciones del Documento',
    vacio: 'Todavía no has anotado nada en este documento.',
    ir: 'Ir al Texto',
    editar: 'Editar',
    borrar: 'Borrar',
    cuerpo: 'Texto de la anotación',
    guardar: 'Guardar',
    cancelar: 'Cancelar',
    confirmar: 'Borrar la Anotación',
    seguro: '¿Borrar esta anotación?',
    nota: 'Nota',
    resaltado: 'Resaltado',
    marcador: 'Marcador',
    reanclada: 'El documento cambió: se recuperó por la cita',
    aproximada: 'Posición aproximada: puede estar en otro párrafo',
    perdida: 'No se pudo colocar en este documento',
    sinPintar: 'No se pudo marcar sobre el texto de la página',
    sinSoporteDeMarcas:
      'Este motor no sabe marcar sobre el texto: las anotaciones solo se ven aquí',
    exportarCitas: 'Citas en',
    exportar: 'Llevarme Estas Anotaciones',
    sinTexto: 'Sin texto'
  },

  libro: {
    unidadCaracteres: ' caracteres',
    familia: 'Tipografía',
    sistema: 'La del sistema',
    serif: 'Con remates',
    'sans-serif': 'Sin remates',
    cuerpo: 'Cuerpo',
    interlineado: 'Interlineado',
    margen: 'Márgenes',
    ancho: 'Ancho de línea',
    alineacion: 'Alineación del texto',
    'alineacion-izquierda': 'A la izquierda',
    'alineacion-centro': 'Centrado',
    'alineacion-derecha': 'A la derecha',
    'alineacion-justificado': 'Justificado',
    paginado: 'Pasar páginas en lugar de desplazar'
  },

  modo: {
    modos: 'Modos de Lectura',
    kit: 'Lectura Accesible',
    emulacion:
      'El brillo y la temperatura se emulan componiendo una capa sobre el texto. No cambian el panel.',
    familia: 'Tipografía',
    ninguna: 'La del libro',
    dislexia: 'Para dislexia',
    bajaVision: 'Para baja visión',
    bionica: 'Lectura biónica',
    regla: 'Regla de lectura',
    focoDeLinea: 'Foco de línea',
    animarPagina: 'Animar el pase de página',
    animarAyuda: 'Con movimiento reducido desaparece por completo.'
  },

  /** Nombres de los modos de lectura. La clave es el identificador del modo. */
  fondoSonoro: {
    fondoSonoro: 'Fondo Sonoro',
    sinFondos:
      'No hay ningún fondo sonoro instalado. Los aporta un paquete de extensión, y no traen audio: el ruido lo sintetiza la aplicación.',
    volumen: 'Volumen',
    temporizador: 'Temporizador',
    sinLimite: 'Hasta que lo pare',
    ayuda: 'Al abrir la aplicación no suena nada hasta que lo eliges.'
  },
  tableros: {
    region: 'Tableros',
    tablero: 'Tablero',
    columnas: 'Columnas del Tablero',
    lista: 'El tablero en orden, columna a columna',
    vacio: 'Sin tarjetas.',
    sinColumnas: 'Este tablero no tiene columnas. Añade la primera para empezar a ordenar notas.',
    lleva: 'Lleva a',
    verLista: 'Ver Como Lista',
    verColumnas: 'Ver en Columnas',
    ayuda:
      'Arrastra una tarjeta a su sitio, o hazlo con el teclado: las flechas recorren el tablero, Alt con las flechas mueve la tarjeta, Supr la quita del tablero e Intro la abre.',
    tableros: 'Tableros',
    nuevo: 'Tablero Nuevo',
    nombreDelTablero: 'Nombre del Tablero',
    crear: 'Crear Tablero',
    columnaInicial1: 'Por Revisar',
    columnaInicial2: 'En Curso',
    columnaInicial3: 'Hecho',
    sinTableros:
      'Un tablero ordena tus notas en columnas con nombre: "Por revisar", "Tesis", "Descartado". Crearlo y borrarlo no toca ninguna nota.',
    eligeTablero: 'Elige un tablero o crea uno nuevo.',
    borrar: 'Borrar Tablero',
    tarjetas: 'tarjetas',
    tarjetasDe: 'de',
    anadirColumna: 'Añadir Columna',
    nombreDeColumna: 'Nombre de la Columna',
    renombrar: 'Renombrar Columna',
    borrarColumna: 'Borrar Columna',
    moverIzquierda: 'Mover a la Izquierda',
    moverDerecha: 'Mover a la Derecha',
    anadirNota: 'Añadir Nota',
    tituloDeNota: 'Título de la Nota Nueva',
    crearNota: 'Crear Nota',
    notaExistente: 'Poner una Nota que Ya Existe',
    elige: 'Elige una nota',
    enlazarTablero: 'Enlazar Otro Tablero',
    cancelar: 'Cancelar',
    guardar: 'Guardar',
    lleno: 'El tablero está lleno. Para seguir, crea otro y enlázalo desde una columna.',
    posicion: 'posición',
    en: 'en'
  },
  modosDeLectura: {
    dia: 'Día',
    sepia: 'Sepia',
    nocturno: 'Nocturno',
    altoContraste: 'Alto Contraste',
    papelMonocromo: 'Papel Monocromo',
    papelAtenuado: 'Papel Atenuado'
  },

  /** Etiquetas de los comandos del lector. La clave es el identificador. */
  comandos: {
    'pagina-siguiente': 'Siguiente',
    'pagina-anterior': 'Anterior',
    'bloque-siguiente': 'Párrafo Siguiente',
    'bloque-anterior': 'Párrafo Anterior',
    'extender-siguiente': 'Extender Hacia Abajo',
    'extender-anterior': 'Extender Hacia Arriba',
    anotar: 'Escribir una Nota',
    consultar: 'Consultar la Selección',
    confirmar: 'Guardar la Nota',
    resaltar: 'Resaltar',
    subrayar: 'Subrayar',
    trazo: 'Dibujar',
    marcador: 'Poner un Marcador',
    'donde-estoy': '¿Dónde estoy?',
    anotaciones: 'Anotaciones',
    voz: 'Leer en Voz Alta',
    indice: 'Índice',
    'ajustes-del-libro': 'Ajustes del Libro',
    'modo-de-lectura': 'Modo de Lectura',
    'fondo-sonoro': 'Fondo Sonoro',
    buscar: 'Buscar en el Documento',
    estudiar: 'Estudiar con IA',
    inmersion: 'Lectura Inmersiva',
    'pantalla-completa': 'Pantalla Completa',
    acercar: 'Acercar',
    alejar: 'Alejar',
    dividir: 'Vista Dividida',
    'cambiar-mitad': 'Cambiar de Mitad',
    'estrechar-mitad': 'Estrechar la Mitad',
    'ensanchar-mitad': 'Ensanchar la Mitad',
    salir: 'Volver'
  },

  consulta: {
    consultando: 'Consultando',
    sinResultados:
      'No hay nada sobre esta palabra en lo consultado. Con un diccionario instalado (Ajustes, Diccionarios) habría definición.',
    sinProveedores:
      'No hay ningún diccionario instalado. Se instalan desde Ajustes, en Diccionarios.',
    fallo: 'No se pudo consultar.',
    otraPalabra: 'Otra Palabra del Párrafo',
    permitir: 'Permitir Esta Consulta',
    seEnvia: 'Se enviará:',
    seEnviaDesconocido: 'el texto que selecciones',
    omitidos: 'Se Quedaron Fuera',
    'omitido-languageUnsupported': 'No cubre el idioma de este documento.',
    'omitido-networkDisabled': 'Está puesto el modo sin red, así que no salió nada.',
    'omitido-networkNotAllowed': 'Falta autorizar este destino.',
    'omitido-failed': 'No respondió.',
    'clase-dictionary': 'Definición',
    'clase-encyclopedia': 'Enciclopedia',
    'clase-translation': 'Traducción'
  },

  voz: {
    controles: 'Lectura en Voz Alta',
    leer: 'Leer en Voz Alta',
    pausar: 'Pausar',
    parar: 'Parar',
    velocidad: 'Velocidad',
    voz: 'Voz',
    local: 'La voz es la del sistema y no sale nada de este equipo.',
    remota:
      'El texto de cada párrafo sale de este equipo hacia el servicio que configuraste. Se cambia en Ajustes.',
    vieneDe: 'La narración viene de',
    loPermitiste: 'Lo autorizaste desde Ajustes.',
    servicioSinNombre: 'un servicio configurado por ti',
    pausarReiniciando: 'Pausar',
    'sin-sintesis':
      'Este sistema no trae síntesis de voz, así que no se puede leer en voz alta aquí.',
    'sin-sintetizador':
      'Falta el sintetizador del sistema. Se instala con el gestor de paquetes: en Arch y derivadas, sudo pacman -S speech-dispatcher espeak-ng; en Debian y Ubuntu, sudo apt install speech-dispatcher espeak-ng; en Fedora, sudo dnf install speech-dispatcher espeak-ng. Después hay que volver a abrir la aplicación.',
    'sin-voces':
      'No hay ninguna voz instalada en el sistema. Instala una y vuelve a abrir este panel.',
    'sin-recursos': 'Este equipo no tiene recursos suficientes para esta voz.',
    lentaEnVivo:
      'Este equipo dice cada párrafo más despacio de lo que dura, así que en vivo se cortará',
    factorMedido: 'factor medido:',
    lentaRecomendado: 'Lo recomendado aquí es preparar el capítulo antes de escucharlo.',
    'sin-servicio':
      'Has elegido una voz de un servicio y todavía no has dicho cuál. Se configura en Ajustes, en Voz.',
    sinSintesis: 'No se puede leer en voz alta en este sistema.'
  },

  estudio: {
    region: 'Estudio',
    secciones: 'Secciones de Estudio',
    notas: 'Notas',
    tarjetas: 'Tarjetas',
    conceptos: 'Conceptos',
    cargarMas: 'Cargar Más',
    nueva: 'Nota Nueva',
    titulo: 'Título',
    cuerpo: 'Contenido',
    guardar: 'Guardar',
    borrar: 'Borrar',
    crear: 'Crear',
    retroenlaces: 'Apuntan Aquí',
    vacio: 'Aún no has escrito ninguna nota.',
    seleccionaNota: 'Selecciona una nota para verla o empieza una nueva.',
    sinRetroenlaces: 'Ninguna nota apunta a esta todavía.',
    ayudaEnlace: 'Escribe [[Título]] para enlazar con otra nota.',
    enlacesSinNota: 'Enlazan con notas que no existen todavía. Crear:',
    anotaciones: 'Anotaciones',
    sinAnotaciones: 'Aún no has anotado nada en ningún documento.',
    pasajes: 'Pasajes que Cita',
    sinPasajes: 'Esta nota no cita ningún pasaje todavía. Se citan desde la anotación.',
    quitarPasaje: 'Quitar este pasaje de la nota',
    citarEn: 'Citar en una nota',
    eligeNota: 'Elige una nota',
    notasQueCitan: 'Notas que Citan Este Pasaje',
    verHistorial: 'Ver las Versiones Anteriores',
    sinCuerpo: 'Sin texto propio',
    seleccionaAnotacion: 'Selecciona una anotación para ver su contenido.',
    todas: 'Todas',
    notaClase: 'Notas',
    resaltados: 'Resaltados',
    marcadores: 'Marcadores',
    subrayados: 'Subrayados',
    trazos: 'Trazos',
    abrir: 'Abrir en el Lector',
    sinArchivo: 'No hay ninguna copia disponible de este documento.',
    deQueLibro: 'De',
    filtro: 'Filtrar por Clase',
    vocabulario: 'Vocabulario',
    sinVocabulario: 'Aún no has consultado ninguna palabra mientras leías.',
    seleccionaPalabra: 'Selecciona una palabra para ver su definición.',
    dondeAparecio: 'Dónde apareció',
    borrarVocabulario: 'Borrar Todo',
    consultadaComo: 'Consultada como',
    consultando: 'Consultando el diccionario'
  },

  descubrir: {
    faltaPermiso: 'Falta autorizar la salida hacia',
    faltaPermisoEnClaro:
      'Este catálogo solo se sirve sin cifrar: quien comparta tu red o esté en el camino puede ver qué miras y qué descargas. Falta autorizarlo sin cifrar:',
    autorizarEnClaro: 'Autorizar Sin Cifrar',
    'error-NEEDS_LOGIN': 'Este catálogo pide usuario y contraseña, y eso todavía no se admite.',
    region: 'Descubrir',
    catalogos: 'Catálogos',
    catalogo: 'Catálogo',
    sinCatalogos: 'No has añadido ningún catálogo. Se añaden en Ajustes.',
    elige: 'Elige un catálogo para verlo.',
    cargando: 'Abriendo el catálogo',
    vacio: 'Este catálogo no tiene nada dentro.',
    abrir: 'Ver Detalles',
    volver: 'Volver',
    descargar: 'Descargar',
    descargando: 'Descargando',
    siguiente: 'Página Siguiente',
    'error-NETWORK_DISABLED': 'Está puesto el modo sin red. Se apaga en Ajustes.',
    'error-CONSENT_REQUIRED':
      'No se ha autorizado la salida hacia este catálogo, o hacia el destino al que redirige. En Ajustes, en Red y Privacidad, se ve a qué anfitrión falta autorizar.',
    autorizar: 'Autorizar',
    'error-NETWORK_FAILURE': 'No se pudo alcanzar el catálogo.',
    'error-DOCUMENT_CORRUPTED': 'Lo que respondió el catálogo no es un feed que se pueda leer.',
    'error-INTERNAL': 'No se pudo abrir el catálogo.',
    'descarga-ok': 'Entró en tu biblioteca:',
    'descarga-repetido': 'Ya estaba en tu biblioteca:',
    'descarga-rechazado': 'No era lo que el catálogo decía, así que no se guardó:'
  },

  buscar: {
    region: 'Búsqueda',
    consulta: 'Buscar',
    catalogo: 'Títulos',
    corpus: 'Dentro de los Libros',
    documento: 'En Este Libro',
    vacio: 'Escribe para buscar.',
    faltanLetras: 'Escribe al menos {n} letras para buscar.',
    sinResultados: 'Ninguna coincidencia.',
    buscando: 'Buscando',
    error: 'No se pudo buscar.',
    'busqueda.pendiente':
      'Todavía se está preparando la búsqueda de este libro. Tarda una vez, y después es inmediata.',
    'busqueda.sin-texto-imagen': 'Este documento son imágenes, así que no hay texto que buscar.',
    'busqueda.sin-capa-de-texto':
      'Este documento no trae texto buscable. Reconocerlo llegará más adelante.',
    'busqueda.archivo-no-disponible':
      'El archivo no está donde el catálogo dice. En Ajustes, en Dónde Viven tus Libros, se arregla.',
    'busqueda.sin-motor': 'Falta el motor que sabe leer este formato.',
    sinCopia: 'No disponible',
    yEnOtras: 'y en otras ediciones:',
    preparar: 'Preparar la Búsqueda'
  },

  importar: {
    elegirCarpeta: 'Elegir Carpeta',
    elegirArchivos: 'Elegir Documentos',
    ruta: 'Ruta',
    ayuda: 'También puedes escribirla, o soltar archivos sobre la ventana.',
    copiar: 'Copiar a la biblioteca',
    copiarAyuda: 'Sin copiar, el documento se queda donde está y solo se registra.',
    importar: 'Importar',
    trabajando: 'Importando',
    resultado: 'Resultado',
    nuevos: 'nuevos',
    yaEstaban: 'ya estaban',
    rechazados: 'rechazados',
    revisados: 'revisados',
    cancelar: 'Cancelar',
    enMarcha: 'Importando. Puedes seguir usando la aplicación.',
    de: 'de',
    cancelada: 'Importación cancelada.',
    quedoAMedias:
      'Quedó una importación a medias. Continuarla no duplica nada: lo que ya entró se reconoce y se salta.',
    continuar: 'Continuar',
    descartar: 'Descartar',
    ultimaImportacion: 'En la última importación',
    seListanLosPrimeros: 'se listan los primeros'
  },

  ajustes: {
    exportarBoton: 'Exportar la Biblioteca',
    extensionInstalada: 'Instalada.',
    extensionPaqueteIncompleto:
      'No se instaló: al paquete le faltan los módulos que su manifiesto declara. Instala el construido, no la carpeta de fuentes.',
    extensionRetirada: 'Retirada del registro',
    guardarDiagnostico: 'Guardar el Diagnóstico',
    guardarAvisosDeIa: 'Guardar los Avisos de IA',
    modoSinContraste:
      'Un modo de lectura de una extensión no llega al contraste mínimo, así que no se ofrece.',
    retiradasSinNovedad: 'Ninguna de tus extensiones está retirada.',
    sinRegistroConfigurado: 'No hay ningún registro configurado, así que no hay a quién preguntar.',
    temaSinContraste: 'Ese tema no se lee bien sobre sí mismo, así que no se ha puesto.'
  },

  /**
   * Lo que la aplicación contesta cuando el usuario hace algo.
   *
   * Una frase corta y en pasado: dice qué quedó hecho, no qué se está
   * intentando. Solo se anuncia lo que **no se ve solo**: cambiar de tema no
   * lleva aviso porque el cambio es la propia pantalla, y anotar sí, porque la
   * anotación se guarda en un panel que puede estar cerrado.
   */
  avisos: {
    poniendoAlDia:
      'Poniendo al día tu biblioteca. Puede tardar un poco la primera vez después de actualizar.',
    extensionDesinstalada: 'Extensión desinstalada. Se recupera en Estados anteriores.',
    modeloInstalado: 'Modelo instalado',
    modeloQuitado: 'Modelo quitado',
    modeloCancelado: 'Descarga cancelada. No ha quedado nada a medias.',
    coleccionCreada: 'Colección creada',
    coleccionBorrada: 'Colección borrada',
    coleccionRenombrada: 'Nombre cambiado',
    anadidoAColeccion: 'Añadido a',
    quitadoDeColeccion: 'Quitado de',
    region: 'Avisos',
    regionFallos: 'Fallos',
    cerrar: 'Cerrar el aviso',
    veces: 'veces',
    notaGuardada: 'Nota guardada',
    notaBorrada: 'Nota borrada',
    anotacionGuardada: 'Nota añadida al documento',
    resaltadoGuardado: 'Resaltado añadido',
    subrayadoGuardado: 'Subrayado añadido',
    trazoGuardado: 'Trazo añadido',
    marcadorPuesto: 'Marcador puesto',
    anotacionEditada: 'Anotación actualizada',
    anotacionBorrada: 'Anotación borrada',
    importacionTerminada: 'Importación terminada',
    audioRetirado: 'Audio preparado borrado por usar una voz retirada:',
    importacionAMedias: 'Quedó una importación a medias. Puedes continuarla desde Importar.',
    indiceEncolado: 'Se rehará la búsqueda. Documentos encolados:',
    pasajeCitado: 'Pasaje citado en la nota',
    metadatosRevisados: 'Propuestas registradas:',
    anotacionesGuardadas: 'Anotaciones guardadas',
    raizReapuntada: 'Tus libros vuelven a estar. Documentos disponibles:',
    fichaCompletada: 'Ficha completada desde el catálogo',
    propuestaAceptada: 'Cambio aplicado a la ficha',
    propuestaRechazada: 'Cambio descartado',
    descargaHecha: 'El documento entró en tu biblioteca',
    descargaRepetida: 'Ese documento ya estaba en tu biblioteca',
    permisoConcedido: 'Permiso concedido',
    permisoRevocado: 'Permiso revocado',
    credencialGuardada: 'Clave guardada en el llavero del sistema',
    enviosDeIaBorrados: 'Registro de envíos borrado',
    reportesDeIaBorrados: 'Avisos borrados',
    reportesDeIaGuardados: 'Avisos guardados',
    credencialBorrada: 'Clave borrada del llavero',
    catalogoAnadido: 'Catálogo añadido',
    catalogoQuitado: 'Catálogo quitado',
    diccionarioInstalado: 'Diccionario instalado',
    diccionarioQuitado: 'Diccionario desinstalado',
    palabraBorrada: 'Palabra quitada del vocabulario',
    vocabularioBorrado: 'Vocabulario vaciado',
    diagnosticoGuardado: 'Diagnóstico guardado',
    bibliotecaExportada: 'Biblioteca exportada. Documentos incluidos:'
  },

  /**
   * Mensajes de los códigos de error canónicos de Rust.
   *
   * Una clave por código, y `desconocido` para lo que llegue sin catalogar. La
   * lista la fija `src-tauri/src/error.rs`, y una prueba comprueba que ningún
   * código se queda sin mensaje en ningún idioma.
   */
  errores: {
    FILE_NOT_FOUND: 'No se encuentra el archivo donde estaba.',
    PATH_NOT_ALLOWED: 'Esa ubicación está fuera de la biblioteca.',
    FORMAT_UNSUPPORTED: 'Este formato no se puede abrir todavía.',
    DOCUMENT_CORRUPTED: 'El archivo está dañado y no se puede leer.',
    DOCUMENT_NOT_FOUND: 'No se encuentra ese documento.',
    STORAGE_FAILURE: 'No se pudo escribir en la biblioteca.',
    SCHEMA_MISMATCH: 'La biblioteca es de una versión distinta de la aplicación.',
    COMPONENT_UNAVAILABLE: 'Esa parte de la aplicación no está disponible ahora mismo.',
    NETWORK_DISABLED: 'La salida de red está apagada.',
    CONSENT_REQUIRED: 'Hace falta tu permiso para consultar ese servicio.',
    NETWORK_FAILURE: 'No se pudo conectar. Lo que hay guardado no cambia.',
    CANCELLED: 'La operación se canceló.',
    STORAGE_BUDGET_REACHED:
      'Lo que la aplicación deriva de tu biblioteca llegó a su tope de disco. Libera sitio, o sube el tope en Ajustes.',
    INTERNAL: 'Algo falló por dentro. No se ha perdido nada de lo guardado.',
    EXTENSION_MANIFEST_INVALID:
      'Esa extensión no está completa: su manifiesto no se puede leer, o declara un módulo compilado que no trae.',
    EXTENSION_API_UNSUPPORTED: 'Esa extensión se hizo para otra versión de la aplicación.',
    EXTENSION_LIMIT_EXCEEDED: 'La extensión se pasó de lo que tenía concedido y se detuvo.',
    EXTENSION_ROLLBACK_UNAVAILABLE: 'No hay un estado anterior al que volver.',
    EXTENSION_SIGNATURE_MISMATCH:
      'Esta actualización no la firma quien firmó la versión que tienes instalada.',
    /** Para un código que llegue sin catalogar. No debería ocurrir nunca. */
    desconocido: 'Algo falló por dentro. No se ha perdido nada de lo guardado.'
  }
} as const;
