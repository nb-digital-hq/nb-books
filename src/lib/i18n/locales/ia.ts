/**
 * Los textos de la IA en Ajustes, en los tres idiomas.
 *
 * # Por qué no están en `ajustes` de cada diccionario
 *
 * Porque el diccionario español entra en el paquete de arranque, que tiene un
 * presupuesto de 250 KB (`performance-budgets.md`, sección 4.11), y estos textos
 * solo los usa Ajustes, que se carga al abrirlo. Con los del proveedor externo
 * dentro, el arranque pasaba a 250,4 KB. Aquí viajan con Ajustes: es la misma
 * regla que partió el paquete por destino, aplicada a los textos.
 *
 * El español manda: los otros dos tienen que tener sus mismas claves, y el tipo lo
 * exige. `i18n.test.ts` comprueba además que ninguna esté vacía ni se haya quedado
 * en español.
 */

import type { Idioma } from '../diccionario';

const es = {
  vozOrigen: 'De Dónde Sale la Voz',
  vozOrigenAyuda:
    'La del sistema ya está instalada y es la más sencilla. La de aquí dentro es más natural y no sale nada del equipo, a cambio de descargar el modelo y de que tu procesador la mueva. La de un servicio suena natural siempre, y manda el texto de cada párrafo a donde tú digas.',
  'vozOrigen-local': 'Del sistema',
  'vozOrigen-neural': 'De aquí dentro',
  'vozOrigen-remote': 'De un servicio',
  vozVoces: 'Voces',
  vozFactor: 'Medido en este equipo: sintetizar tarda esto por cada segundo de audio:',
  vozListaParaNarrar: 'Lista: este equipo la mueve mientras lees.',
  'vozMotivo-NO_INSTALADA': 'Falta descargar el modelo.',
  'vozMotivo-NO_CABE':
    'No cabe en el tope de memoria de la IA. Súbelo en IA y Modelos o elige otra voz.',
  'vozMotivo-DEMASIADO_LENTA':
    'Este equipo tarda más en decir un párrafo de lo que dura: se puede narrar en vivo, pero se cortará. Lo recomendado aquí es preparar el capítulo o el libro antes de escucharlo.',
  'vozMotivo-NO_SE_CARGA': 'El modelo está y no se deja cargar. Vuelve a descargarlo.',
  vozNoInstalada: 'El modelo de voz no está descargado.',
  vozQueNoSale: 'Se descarga una vez y después no sale nada de este equipo: la voz se genera aquí.',
  modelos: 'IA y Modelos',
  modelosAyuda:
    'La IA es opcional y, de fábrica, funciona en este equipo: lo que lees no sale de aquí salvo que configures un proveedor externo. Para usarla hace falta descargar un modelo, una sola vez.',
  modelosAvisoDeRendimiento:
    'En equipos menos potentes, la IA trabajando de fondo puede ralentizar la aplicación mientras lees. Se detiene sola al desplazarte o pasar página, y puedes pausarla o quitarla cuando quieras.',
  modelosSinMaquina:
    'Este equipo tiene poca memoria para la IA local: puedes descargar un modelo igual, y cada uno dice abajo lo que es probable que pase. El resto de la aplicación funciona igual.',
  modeloNoCabeEnElTope:
    'Con el tope de memoria elegido este modelo no se carga, y la IA queda sin usar. Necesita',
  modelosRecursos: 'Lo que Puede Gastar',
  iaCalculaEn: 'En este equipo la IA calcula en',
  'iaAcelerador-cpu': 'el procesador',
  'iaAcelerador-cuda': 'la gráfica NVIDIA',
  'iaAcelerador-metal': 'la gráfica de Apple',
  iaEdicionRapida:
    'Esta edición no usa las instrucciones rápidas de tu procesador (AVX2); la edición rápida de NB Books genera unas diez veces más deprisa en él.',
  modeloPorQueRecomendado:
    ' Es el que mejor salió en las mediciones de los que este equipo aguanta: los más grandes piden más memoria o más núcleos de los que tiene, o citan peor.',
  modeloPrevision: 'En este equipo, cada resumen o juego de tarjetas tardaría',
  modeloPrevisionEstimada: '(estimado a partir de lo medido en otro equipo).',
  modeloMenosDeUnMinuto: 'menos de un minuto',
  modeloUnos: 'unos',
  modeloMinutos: 'minutos',
  'modeloAviso-SLOW_BUILD':
    'Esta edición de NB Books no usa las instrucciones rápidas de tu procesador (AVX2): funciona, pero unas diez veces más despacio. La edición rápida lo arregla.',
  'modeloAviso-BELOW_RECOMMENDED':
    'Tu equipo está por debajo de lo recomendado para este modelo: funcionará, más despacio o con el equipo más cargado mientras genera.',
  'modeloAviso-EXCEEDS_MACHINE_MEMORY':
    'Este modelo pide más memoria de la que tiene el equipo en total. Se puede elegir, pero lo más probable es que el sistema cierre la aplicación al cargarlo.',
  'modeloAviso-CITES_WORSE':
    'Medido: escribe con más soltura, pero se aleja del texto, y al comprobar sus citas pierde más que el recomendado.',
  modeloConfirmarMemoria:
    'Lo más probable es que el sistema cierre NB Books al cargar este modelo, porque pide más memoria de la que tiene el equipo. Tu lectura está guardada y no se pierde nada, pero se cerrará lo que tengas abierto.',
  modeloUsarIgualmente: 'Usarlo Igualmente',
  modeloSubirTope: 'Subir el Tope a',
  modelosRecursosAyuda:
    'La IA nunca usa todos los núcleos y calcula con prioridad baja, para que leer siga siendo fluido mientras trabaja.',
  modelosHilos: 'Hilos',
  modelosHilosAyuda: 'Cuántos núcleos del procesador puede ocupar a la vez. Nunca todos.',
  modelosHilosAutomatico: 'Automático',
  modelosDe: 'de',
  modelosMemoriaTope: 'Memoria para un modelo',
  modelosMemoriaAyuda:
    'Un modelo que necesite más no se carga, y con menos memoria calcula en tandas más pequeñas.',
  modelosEnPausaBateria:
    'El trabajo de fondo de la IA espera a que haya más batería o se enchufe el equipo. Lo que pidas sigue funcionando.',
  modelosEnPausaCalor:
    'El trabajo de fondo de la IA espera a que el equipo se enfríe. Lo que pidas sigue funcionando.',
  iaActivo: 'Proveedor activo:',
  iaEn: 'en',
  iaActivoSale: 'Lo que le pidas a la IA sale del equipo hacia ese servicio.',
  iaActivoLocal: 'solo este equipo. Nada de lo que pidas a la IA sale de aquí.',
  'modeloRasgo-fiel': 'Se pega al texto: sus citas resisten la comprobación.',
  'modeloRasgo-prosa': 'Escribe mejor, pero se aleja del texto y pierde citas al comprobarlas.',
  'modeloRasgo-vectores': 'No genera texto: prepara la búsqueda por significado.',
  modeloComoEscribe: 'Cómo Escribe',
  modeloEquipo: 'Se Recomienda Desde',
  modeloEquipoDesde: 'un equipo con',
  modeloEquipoY: 'de memoria y',
  modeloEquipoNucleos: 'núcleos',
  modeloRecomendado: 'Recomendado para este equipo.',
  modeloEnUso: 'Es el que genera ahora.',
  modeloUsarEste: 'Usar Este',
  modeloPara: 'Para',
  modeloParaBuscar: 'Buscar por significado',
  modeloParaGenerar: 'Resúmenes, tarjetas y conceptos',
  iaGenerador: 'Quién Genera',
  iaGeneradorAyuda:
    'Los resúmenes, las tarjetas y los conceptos del Lector los genera el modelo local o el proveedor externo, el que elijas. Si el elegido no está disponible, no se usa el otro en su lugar.',
  'iaGenerador-local': 'Modelo Local',
  'iaGenerador-external': 'Proveedor Externo',
  iaExterna: 'Proveedor Externo',
  iaExternaAyuda:
    'Opcional. Con un proveedor externo, las funciones que generan texto mandan a ese servicio los fragmentos que necesiten, nunca el documento entero. Los vectores de búsqueda se calculan siempre aquí.',
  iaProtocolo: 'Proveedor',
  'iaProtocolo-none': 'Ninguno',
  'iaProtocolo-openai': 'Compatible con OpenAI',
  'iaProtocolo-anthropic': 'Anthropic',
  iaServidor: 'Servidor',
  iaServidorEjemplo: 'api.openai.com o localhost:11434',
  iaDireccionNoVale:
    'Esta dirección no vale: sin cifrar solo se admite hacia este equipo o la red propia, por dirección numérica.',
  iaModelo: 'Modelo',
  iaClave: 'Clave del servicio',
  iaClaveAyuda:
    'Se guarda en el llavero del sistema, nunca en la base de datos, y solo se envía a este servidor. Un servidor en tu propio equipo no la necesita.',
  iaProbar: 'Probar',
  iaAntesDelPrimerEnvio: 'Antes del Primer Envío',
  iaSeEnviaraA: 'Esto es exactamente lo que se enviará a',
  iaTamanoDelEnvio: 'Tamaño del envío: {caracteres} caracteres (~{tokens} tokens).',
  iaRetencion:
    'NB Books no controla qué hace con ello quien lo recibe. La clave no aparece aquí porque viaja aparte.',
  iaAutorizarYEnviar: 'Autorizar y Enviar',
  iaRespondio: 'Respondió:',
  iaRechazo: 'El servicio rechazó la petición con el código',
  iaRechazoClave: 'Suele ser la clave: falta, no vale o no tiene permiso.',
  iaRechazoModelo: 'Suele ser el modelo o la dirección: no existe en ese servidor.',
  iaRechazoLimite: 'Es un límite de uso del servicio.',
  iaRechazoOtro: 'El servicio no dijo por qué.',
  iaSinConfigurar: 'Falta el servidor o el modelo.',
  iaEnvios: 'Envíos Registrados',
  iaEnviosAyuda:
    'Todo lo que ha salido hacia un proveedor externo, entero y tal como salió. Se guarda solo en este equipo.',
  iaSinEnvios: 'No se ha enviado nada.',
  iaSinRespuesta: 'sin respuesta',
  iaBorrarEnvios: 'Borrar el Registro',
  iaReportes: 'Lo Marcado como Incorrecto',
  iaReportesAyuda:
    'Lo que has marcado mal en una salida generada, con la plantilla y el modelo que la hicieron. Se queda aquí: no se manda a ninguna parte, y no lleva el texto de tus libros.',
  iaSinReportes: 'No has marcado nada.',
  iaBorrarReportes: 'Borrar lo Marcado',
  iaQuitarReporte: 'Quitar Este Aviso',
  iaExportarReportes: 'Guardar en un Archivo',
  'iaMotivo-falso': 'Dice algo que el libro no dice',
  'iaMotivo-no_viene_a_cuento': 'No viene a cuento',
  'iaMotivo-incompleto': 'Incompleto',
  'iaMotivo-mal_escrito': 'Mal escrito',
  'iaMotivo-otro': 'Otra cosa',
  modeloLicencia: 'Licencia',
  modeloDescarga: 'Descarga',
  modeloMemoria: 'Memoria',
  modeloIdiomas: 'Idiomas',
  modeloDescargando: 'Descargando el Modelo',
  modeloCancelar: 'Cancelar la Descarga',
  modeloInstalado: 'Instalado y verificado.',
  modeloQuitar: 'Quitar el Modelo',
  modeloAntesDeDescargar: 'Antes de Descargar',
  modeloDeQuien: 'El modelo es de',
  modeloBajoLicencia: ', no de NB Books, y se distribuye bajo la licencia',
  modeloSeDescargaDe: 'Se descarga de',
  modeloQueSale: 'Qué sale del equipo:',
  modeloNoCabe: 'No cabe en el disco: hacen falta',
  modeloNoCabeLibres: ' y quedan libres',
  modeloAutorizarYDescargar: 'Autorizar y Descargar',
  modeloNoDescargar: 'Ahora No',
  modeloRoto: 'Estaba instalado y ya no está entero. Se puede volver a descargar.',
  modeloReinstalar: 'Volver a Descargar',
  modeloDescargar: 'Descargar',
  modeloSigueDesde: 'La descarga sigue desde',
  modeloRedDeDistribucion:
    ', la red de distribución de quien sirve el modelo, y necesita su propio permiso.',
  modeloAutorizarYSeguir: 'Autorizar y Seguir',
  modeloNoCoincide:
    'Lo descargado no coincide con lo que se revisó, así que no se ha instalado nada.'
};

export type ClaveDeIa = keyof typeof es;

export const TEXTOS_DE_IA: Record<Idioma, Record<ClaveDeIa, string>> = {
  es,
  en: {
    vozOrigen: 'Where the Voice Comes From',
    vozOrigenAyuda:
      'The system one is already installed and sounds like a machine. The one in here sounds good and nothing leaves this computer, in exchange for downloading the model and for your processor being able to run it. A service one always sounds good, and sends the text of every paragraph wherever you say.',
    'vozOrigen-local': 'From the system',
    'vozOrigen-neural': 'From in here',
    'vozOrigen-remote': 'From a service',
    vozVoces: 'Voices',
    vozFactor: 'Measured on this computer: synthesis takes this much per second of audio:',
    vozListaParaNarrar: 'Ready: this computer runs it while you read.',
    'vozMotivo-NO_INSTALADA': 'The model still has to be downloaded.',
    'vozMotivo-NO_CABE':
      'It does not fit the AI memory cap. Raise it under AI and Models, or pick another voice.',
    'vozMotivo-DEMASIADO_LENTA':
      'This computer takes longer to say a paragraph than the paragraph lasts: live narration works, but it will stutter. What we recommend here is preparing the chapter or the book before listening.',
    'vozMotivo-NO_SE_CARGA': 'The model is there and will not load. Download it again.',
    vozNoInstalada: 'The voice model has not been downloaded.',
    vozQueNoSale:
      'It is downloaded once and after that nothing leaves this computer: the voice is generated here.',
    modelos: 'AI and Models',
    modelosAyuda:
      'AI is optional and, by default, runs on this device: what you read does not leave it unless you set up an external provider. To use it, a model has to be downloaded, once.',
    modelosAvisoDeRendimiento:
      'On less powerful computers, AI working in the background can slow the application down while you read. It pauses on its own when you scroll or turn the page, and you can pause or remove it at any time.',
    modelosSinMaquina:
      'This device is short on memory for local AI: you can still download a model, and each one says below what is likely to happen. The rest of the application works the same.',
    modeloNoCabeEnElTope:
      'With the chosen memory limit this model does not load, and AI stays unused. It needs',
    modelosRecursos: 'What It Can Use',
    iaCalculaEn: 'On this machine the AI computes on',
    'iaAcelerador-cpu': 'the processor',
    'iaAcelerador-cuda': 'the NVIDIA graphics card',
    'iaAcelerador-metal': 'the Apple graphics',
    iaEdicionRapida:
      "This edition does not use your processor's fast instructions (AVX2); the fast edition of NB Books generates about ten times faster on it.",
    modeloPorQueRecomendado:
      ' It is the one that measured best among those this machine can run: the larger ones need more memory or cores than it has, or cite worse.',
    modeloPrevision: 'On this machine, each summary or set of cards would take',
    modeloPrevisionEstimada: '(estimated from what was measured on another machine).',
    modeloMenosDeUnMinuto: 'less than a minute',
    modeloUnos: 'about',
    modeloMinutos: 'minutes',
    'modeloAviso-SLOW_BUILD':
      "This edition of NB Books does not use your processor's fast instructions (AVX2): it works, but about ten times slower. The fast edition fixes that.",
    'modeloAviso-BELOW_RECOMMENDED':
      'Your machine is below what is recommended for this model: it will work, more slowly or with the machine busier while it generates.',
    'modeloAviso-EXCEEDS_MACHINE_MEMORY':
      'This model needs more memory than the machine has in total. You can choose it, but the system will most likely close the app when it loads.',
    'modeloAviso-CITES_WORSE':
      'Measured: it writes more fluently, but drifts from the text, and loses more citations than the recommended one when they are checked.',
    modeloConfirmarMemoria:
      'The system will most likely close NB Books when it loads this model, because it needs more memory than the machine has. Your reading is saved and nothing is lost, but whatever you have open will close.',
    modeloUsarIgualmente: 'Use It Anyway',
    modeloSubirTope: 'Raise the Limit to',
    modelosRecursosAyuda:
      'AI never takes every core and runs at low priority, so reading stays smooth while it works.',
    modelosHilos: 'Threads',
    modelosHilosAyuda: 'How many processor cores it can use at once. Never all of them.',
    modelosHilosAutomatico: 'Automatic',
    modelosDe: 'of',
    modelosMemoriaTope: 'Memory for a model',
    modelosMemoriaAyuda:
      'A model that needs more does not load, and with less memory it works in smaller batches.',
    modelosEnPausaBateria:
      'Background AI work waits for more battery or for the computer to be plugged in. What you ask for still works.',
    modelosEnPausaCalor:
      'Background AI work waits for the computer to cool down. What you ask for still works.',
    iaActivo: 'Active provider:',
    iaEn: 'on',
    iaActivoSale: 'What you ask the AI leaves this computer for that service.',
    iaActivoLocal: 'this computer only. Nothing you ask the AI leaves it.',
    'modeloRasgo-fiel': 'Stays close to the text: its citations survive checking.',
    'modeloRasgo-prosa':
      'Writes better, but drifts from the text and loses citations when checked.',
    'modeloRasgo-vectores': 'Does not generate text: it prepares search by meaning.',
    modeloComoEscribe: 'How It Writes',
    modeloEquipo: 'Recommended From',
    modeloEquipoDesde: 'a machine with',
    modeloEquipoY: 'of memory and',
    modeloEquipoNucleos: 'cores',
    modeloRecomendado: 'Recommended for this machine.',
    modeloEnUso: 'This is the one generating now.',
    modeloUsarEste: 'Use This One',
    modeloPara: 'For',
    modeloParaBuscar: 'Searching by meaning',
    modeloParaGenerar: 'Summaries, flashcards and concepts',
    iaGenerador: 'Who Generates',
    iaGeneradorAyuda:
      'Summaries, flashcards and concepts in the Reader are generated by the local model or the external provider, whichever you choose. If the chosen one is unavailable, the other is not used instead.',
    'iaGenerador-local': 'Local Model',
    'iaGenerador-external': 'External Provider',
    iaExterna: 'External Provider',
    iaExternaAyuda:
      'Optional. With an external provider, features that generate text send that service the passages they need, never the whole document. Search vectors are always computed here.',
    iaProtocolo: 'Provider',
    'iaProtocolo-none': 'None',
    'iaProtocolo-openai': 'OpenAI compatible',
    'iaProtocolo-anthropic': 'Anthropic',
    iaServidor: 'Server',
    iaServidorEjemplo: 'api.openai.com or localhost:11434',
    iaDireccionNoVale:
      'This address is not allowed: unencrypted connections are only accepted to this computer or your own network, by numeric address.',
    iaModelo: 'Model',
    iaClave: 'Service key',
    iaClaveAyuda:
      'Stored in the system keyring, never in the database, and only sent to this server. A server on your own computer does not need one.',
    iaProbar: 'Test',
    iaAntesDelPrimerEnvio: 'Before the First Request',
    iaSeEnviaraA: 'This is exactly what will be sent to',
    iaTamanoDelEnvio: 'Payload size: {caracteres} characters (~{tokens} tokens).',
    iaRetencion:
      'NB Books does not control what the recipient does with it. The key is not shown here because it travels separately.',
    iaAutorizarYEnviar: 'Allow and Send',
    iaRespondio: 'It answered:',
    iaRechazo: 'The service rejected the request with code',
    iaRechazoClave: 'Usually the key: missing, invalid or without permission.',
    iaRechazoModelo: 'Usually the model or the address: it does not exist on that server.',
    iaRechazoLimite: 'It is a usage limit of the service.',
    iaRechazoOtro: 'The service did not say why.',
    iaSinConfigurar: 'The server or the model is missing.',
    iaEnvios: 'Logged Requests',
    iaEnviosAyuda:
      'Everything that has been sent to an external provider, in full and exactly as it left. Kept only on this computer.',
    iaSinEnvios: 'Nothing has been sent.',
    iaSinRespuesta: 'no response',
    iaBorrarEnvios: 'Clear the Log',
    iaReportes: 'What You Flagged as Wrong',
    iaReportesAyuda:
      'What you flagged in a generated output, with the template and model that made it. It stays here: nothing is sent anywhere, and it does not carry the text of your books.',
    iaSinReportes: 'You have not flagged anything.',
    iaBorrarReportes: 'Delete What Was Flagged',
    iaQuitarReporte: 'Remove This Flag',
    iaExportarReportes: 'Save to a File',
    'iaMotivo-falso': 'Says something the book does not say',
    'iaMotivo-no_viene_a_cuento': 'Beside the point',
    'iaMotivo-incompleto': 'Incomplete',
    'iaMotivo-mal_escrito': 'Badly written',
    'iaMotivo-otro': 'Something else',
    modeloLicencia: 'Licence',
    modeloDescarga: 'Download',
    modeloMemoria: 'Memory',
    modeloIdiomas: 'Languages',
    modeloDescargando: 'Downloading the Model',
    modeloCancelar: 'Cancel the Download',
    modeloInstalado: 'Installed and verified.',
    modeloQuitar: 'Remove the Model',
    modeloAntesDeDescargar: 'Before Downloading',
    modeloDeQuien: 'The model belongs to',
    modeloBajoLicencia: ', not to NB Books, and is distributed under the licence',
    modeloSeDescargaDe: 'It is downloaded from',
    modeloQueSale: 'What leaves this device:',
    modeloNoCabe: 'It does not fit on the disk: it needs',
    modeloNoCabeLibres: ' and there are free',
    modeloAutorizarYDescargar: 'Allow and Download',
    modeloNoDescargar: 'Not Now',
    modeloRoto: 'It was installed and is no longer complete. It can be downloaded again.',
    modeloReinstalar: 'Download Again',
    modeloDescargar: 'Download',
    modeloSigueDesde: 'The download continues from',
    modeloRedDeDistribucion:
      ', the distribution network of whoever serves the model, and it needs its own permission.',
    modeloAutorizarYSeguir: 'Allow and Continue',
    modeloNoCoincide:
      'What was downloaded does not match what was reviewed, so nothing has been installed.'
  },
  pt: {
    vozOrigen: 'De Onde Vem a Voz',
    vozOrigenAyuda:
      'A do sistema já está instalada e soa a máquina. A daqui de dentro soa bem e nada sai deste computador, em troca de baixar o modelo e de o teu processador dar conta dela. A de um serviço soa sempre bem, e manda o texto de cada parágrafo para onde tu disseres.',
    'vozOrigen-local': 'Do sistema',
    'vozOrigen-neural': 'Daqui de dentro',
    'vozOrigen-remote': 'De um serviço',
    vozVoces: 'Vozes',
    vozFactor: 'Medido neste computador: sintetizar demora isto por cada segundo de áudio:',
    vozListaParaNarrar: 'Pronta: este computador dá conta dela enquanto lês.',
    'vozMotivo-NO_INSTALADA': 'Falta baixar o modelo.',
    'vozMotivo-NO_CABE':
      'Não cabe no limite de memória da IA. Aumenta-o em IA e Modelos ou escolhe outra voz.',
    'vozMotivo-DEMASIADO_LENTA':
      'Este computador demora mais a dizer um parágrafo do que o parágrafo dura: dá para narrar ao vivo, mas vai cortar. O recomendado aqui é preparar o capítulo ou o livro antes de ouvir.',
    'vozMotivo-NO_SE_CARGA': 'O modelo está e não se deixa carregar. Baixa-o outra vez.',
    vozNoInstalada: 'O modelo de voz não está baixado.',
    vozQueNoSale: 'Baixa-se uma vez e depois nada sai deste computador: a voz é gerada aqui.',
    modelos: 'IA e Modelos',
    modelosAyuda:
      'A IA é opcional e, de fábrica, funciona neste equipamento: o que você lê não sai daqui, a menos que configure um fornecedor externo. Para usá-la é preciso baixar um modelo, uma única vez.',
    modelosAvisoDeRendimiento:
      'Em computadores menos potentes, a IA a trabalhar em segundo plano pode tornar a aplicação mais lenta enquanto lê. Pára sozinha ao deslocar ou virar a página, e pode pausá-la ou retirá-la quando quiser.',
    modelosSinMaquina:
      'Este equipamento tem pouca memória para a IA local: podes baixar um modelo na mesma, e cada um diz abaixo o que é provável que aconteça. O resto do aplicativo funciona igual.',
    modeloNoCabeEnElTope:
      'Com o limite de memória escolhido este modelo não é carregado, e a IA fica sem uso. Precisa de',
    modelosRecursos: 'O Que Pode Gastar',
    iaCalculaEn: 'Neste equipamento a IA calcula',
    'iaAcelerador-cpu': 'no processador',
    'iaAcelerador-cuda': 'na placa gráfica NVIDIA',
    'iaAcelerador-metal': 'na gráfica da Apple',
    iaEdicionRapida:
      'Esta edição não usa as instruções rápidas do teu processador (AVX2); a edição rápida do NB Books gera umas dez vezes mais depressa nele.',
    modeloPorQueRecomendado:
      ' É o que melhor se saiu nas medições entre os que este equipamento aguenta: os maiores pedem mais memória ou mais núcleos do que ele tem, ou citam pior.',
    modeloPrevision: 'Neste equipamento, cada resumo ou conjunto de cartões demoraria',
    modeloPrevisionEstimada: '(estimado a partir do que se mediu noutro equipamento).',
    modeloMenosDeUnMinuto: 'menos de um minuto',
    modeloUnos: 'uns',
    modeloMinutos: 'minutos',
    'modeloAviso-SLOW_BUILD':
      'Esta edição do NB Books não usa as instruções rápidas do teu processador (AVX2): funciona, mas umas dez vezes mais devagar. A edição rápida resolve isso.',
    'modeloAviso-BELOW_RECOMMENDED':
      'O teu equipamento está abaixo do recomendado para este modelo: vai funcionar, mais devagar ou com o equipamento mais ocupado enquanto gera.',
    'modeloAviso-EXCEEDS_MACHINE_MEMORY':
      'Este modelo pede mais memória do que o equipamento tem no total. Pode ser escolhido, mas o mais provável é o sistema fechar a aplicação ao carregá-lo.',
    'modeloAviso-CITES_WORSE':
      'Medido: escreve com mais fluidez, mas afasta-se do texto, e ao verificar as citações perde mais do que o recomendado.',
    modeloConfirmarMemoria:
      'O mais provável é o sistema fechar o NB Books ao carregar este modelo, porque pede mais memória do que o equipamento tem. A tua leitura está guardada e não se perde nada, mas fecha-se o que tiveres aberto.',
    modeloUsarIgualmente: 'Usar na Mesma',
    modeloSubirTope: 'Subir o Limite para',
    modelosRecursosAyuda:
      'A IA nunca usa todos os núcleos e calcula com prioridade baixa, para que a leitura continue fluida enquanto trabalha.',
    modelosHilos: 'Threads',
    modelosHilosAyuda: 'Quantos núcleos do processador pode ocupar ao mesmo tempo. Nunca todos.',
    modelosHilosAutomatico: 'Automático',
    modelosDe: 'de',
    modelosMemoriaTope: 'Memória para um modelo',
    modelosMemoriaAyuda:
      'Um modelo que precise de mais não é carregado, e com menos memória calcula em lotes menores.',
    modelosEnPausaBateria:
      'O trabalho de fundo da IA espera por mais bateria ou que o computador seja ligado à corrente. O que pedir continua a funcionar.',
    modelosEnPausaCalor:
      'O trabalho de fundo da IA espera que o computador arrefeça. O que pedir continua a funcionar.',
    iaActivo: 'Fornecedor ativo:',
    iaEn: 'em',
    iaActivoSale: 'O que pedir à IA sai do computador para esse serviço.',
    iaActivoLocal: 'apenas este computador. Nada do que pedir à IA sai daqui.',
    'modeloRasgo-fiel': 'Cola-se ao texto: as suas citações resistem à verificação.',
    'modeloRasgo-prosa': 'Escreve melhor, mas afasta-se do texto e perde citações ao verificá-las.',
    'modeloRasgo-vectores': 'Não gera texto: prepara a pesquisa por significado.',
    modeloComoEscribe: 'Como Escreve',
    modeloEquipo: 'Recomendado A Partir De',
    modeloEquipoDesde: 'um equipamento com',
    modeloEquipoY: 'de memória e',
    modeloEquipoNucleos: 'núcleos',
    modeloRecomendado: 'Recomendado para este equipamento.',
    modeloEnUso: 'É o que gera agora.',
    modeloUsarEste: 'Usar Este',
    modeloPara: 'Para',
    modeloParaBuscar: 'Pesquisar por significado',
    modeloParaGenerar: 'Resumos, cartões e conceitos',
    iaGenerador: 'Quem Gera',
    iaGeneradorAyuda:
      'Os resumos, cartões e conceitos do Leitor são gerados pelo modelo local ou pelo fornecedor externo, o que você escolher. Se o escolhido não estiver disponível, o outro não é usado no lugar.',
    'iaGenerador-local': 'Modelo Local',
    'iaGenerador-external': 'Fornecedor Externo',
    iaExterna: 'Fornecedor Externo',
    iaExternaAyuda:
      'Opcional. Com um fornecedor externo, as funções que geram texto enviam a esse serviço os fragmentos de que precisam, nunca o documento inteiro. Os vetores de pesquisa são sempre calculados aqui.',
    iaProtocolo: 'Fornecedor',
    'iaProtocolo-none': 'Nenhum',
    'iaProtocolo-openai': 'Compatível com OpenAI',
    'iaProtocolo-anthropic': 'Anthropic',
    iaServidor: 'Servidor',
    iaServidorEjemplo: 'api.openai.com ou localhost:11434',
    iaDireccionNoVale:
      'Este endereço não é válido: sem cifra só se aceita para este computador ou a rede própria, por endereço numérico.',
    iaModelo: 'Modelo',
    iaClave: 'Chave do serviço',
    iaClaveAyuda:
      'Guarda-se no chaveiro do sistema, nunca na base de dados, e só se envia a este servidor. Um servidor no seu próprio computador não precisa dela.',
    iaProbar: 'Testar',
    iaAntesDelPrimerEnvio: 'Antes do Primeiro Envio',
    iaSeEnviaraA: 'Isto é exatamente o que será enviado para',
    iaTamanoDelEnvio: 'Tamanho do envio: {caracteres} caracteres (~{tokens} tokens).',
    iaRetencion:
      'O NB Books não controla o que quem o recebe faz com isso. A chave não aparece aqui porque viaja à parte.',
    iaAutorizarYEnviar: 'Autorizar e Enviar',
    iaRespondio: 'Respondeu:',
    iaRechazo: 'O serviço recusou o pedido com o código',
    iaRechazoClave: 'Costuma ser a chave: falta, não é válida ou não tem permissão.',
    iaRechazoModelo: 'Costuma ser o modelo ou o endereço: não existe nesse servidor.',
    iaRechazoLimite: 'É um limite de uso do serviço.',
    iaRechazoOtro: 'O serviço não disse porquê.',
    iaSinConfigurar: 'Falta o servidor ou o modelo.',
    iaEnvios: 'Envios Registados',
    iaEnviosAyuda:
      'Tudo o que saiu para um fornecedor externo, inteiro e tal como saiu. Guarda-se só neste computador.',
    iaSinEnvios: 'Não foi enviado nada.',
    iaSinRespuesta: 'sem resposta',
    iaBorrarEnvios: 'Apagar o Registo',
    iaReportes: 'O Que Marcaste como Errado',
    iaReportesAyuda:
      'O que marcaste numa saída gerada, com o guião e o modelo que a fizeram. Fica aqui: não se manda para lado nenhum e não leva o texto dos teus livros.',
    iaSinReportes: 'Não marcaste nada.',
    iaBorrarReportes: 'Apagar o Marcado',
    iaQuitarReporte: 'Remover Este Aviso',
    iaExportarReportes: 'Guardar num Ficheiro',
    'iaMotivo-falso': 'Diz algo que o livro não diz',
    'iaMotivo-no_viene_a_cuento': 'Não vem ao caso',
    'iaMotivo-incompleto': 'Incompleto',
    'iaMotivo-mal_escrito': 'Mal escrito',
    'iaMotivo-otro': 'Outra coisa',
    modeloLicencia: 'Licença',
    modeloDescarga: 'Download',
    modeloMemoria: 'Memória',
    modeloIdiomas: 'Idiomas',
    modeloDescargando: 'Baixando o Modelo',
    modeloCancelar: 'Cancelar o Download',
    modeloInstalado: 'Instalado e verificado.',
    modeloQuitar: 'Remover o Modelo',
    modeloAntesDeDescargar: 'Antes de Baixar',
    modeloDeQuien: 'O modelo é de',
    modeloBajoLicencia: ', não do NB Books, e é distribuído sob a licença',
    modeloSeDescargaDe: 'É baixado de',
    modeloQueSale: 'O que sai do equipamento:',
    modeloNoCabe: 'Não cabe no disco: são necessários',
    modeloNoCabeLibres: ' e restam livres',
    modeloAutorizarYDescargar: 'Autorizar e Baixar',
    modeloNoDescargar: 'Agora Não',
    modeloRoto: 'Estava instalado e já não está completo. Pode ser baixado de novo.',
    modeloReinstalar: 'Baixar de Novo',
    modeloDescargar: 'Baixar',
    modeloSigueDesde: 'O download continua a partir de',
    modeloRedDeDistribucion:
      ', a rede de distribuição de quem serve o modelo, e precisa da sua própria permissão.',
    modeloAutorizarYSeguir: 'Autorizar e Continuar',
    modeloNoCoincide:
      'O que foi baixado não coincide com o que foi revisado, então nada foi instalado.'
  }
};
