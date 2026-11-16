/**
 * Los textos de la preparación de la búsqueda por significado, en los
 * tres idiomas.
 *
 * Aparte de los diccionarios por lo mismo que `ia.ts`: solo los usan piezas que se
 * cargan a demanda, y el diccionario español entra en el paquete de arranque, que
 * tiene un presupuesto de 250 KB.
 */

import type { Idioma } from '../diccionario';

const es = {
  titulo: 'Por Significado',
  sinModelo:
    'Para buscar por significado hace falta el modelo de IA, que se descarga en Ajustes, en IA y Modelos.',
  listo: 'Preparado para buscar por significado:',
  fragmentos: 'fragmentos',
  documentos: 'documentos',
  de: 'de',
  costeDisco: 'Prepararlo ocupa unos',
  costeTiempo: 'en disco y lleva unos',
  costeProcesador: 'de procesador',
  medido: 'medido en este equipo',
  supuesto: 'calculado sobre lo que suele tardar',
  para: 'para',
  sinIndexar: 'Algunos todavía no están indexados, así que sus fragmentos se han supuesto.',
  noCabe:
    'No cabe en el espacio reservado para lo derivado. Se puede ampliar en Ajustes, en Índice de Búsqueda.',
  preparar: 'Preparar',
  preparando: 'Preparando',
  enCola: 'documentos más esperando',
  pausar: 'Pausar',
  reanudar: 'Reanudar',
  cancelar: 'Cancelar',
  'motivo-USER_PAUSED': 'En pausa.',
  'motivo-READER_ACTIVE': 'Esperando a que dejes de pasar página.',
  'motivo-AI_SUSPENDED_BATTERY': 'En pausa hasta que haya más batería o se enchufe el equipo.',
  'motivo-AI_SUSPENDED_HEAT': 'En pausa hasta que el equipo se enfríe.',
  excluir: 'Excluir de la búsqueda por significado',
  excluido: 'Excluido: no se prepara, y lo que estuviera preparado se ha borrado.',
  origenExacta: 'Exacta',
  origenSignificado: 'Por significado',
  origenAmbas: 'Exacta y por significado'
};

export type ClaveDeSignificado = keyof typeof es;

export const TEXTOS_DE_SIGNIFICADO: Record<Idioma, Record<ClaveDeSignificado, string>> = {
  es,
  en: {
    titulo: 'By Meaning',
    sinModelo:
      'Searching by meaning needs the AI model, which is downloaded in Settings, under AI and Models.',
    listo: 'Ready to search by meaning:',
    fragmentos: 'passages',
    documentos: 'documents',
    de: 'of',
    costeDisco: 'Preparing it takes about',
    costeTiempo: 'of disk and about',
    costeProcesador: 'of processor time',
    medido: 'measured on this computer',
    supuesto: 'based on how long it usually takes',
    para: 'for',
    sinIndexar: 'Some are not indexed yet, so their passages have been estimated.',
    noCabe:
      'It does not fit in the space reserved for derived data. It can be raised in Settings, under Search Index.',
    preparar: 'Prepare',
    preparando: 'Preparing',
    enCola: 'more documents waiting',
    pausar: 'Pause',
    reanudar: 'Resume',
    cancelar: 'Cancel',
    'motivo-USER_PAUSED': 'Paused.',
    'motivo-READER_ACTIVE': 'Waiting for you to stop turning pages.',
    'motivo-AI_SUSPENDED_BATTERY':
      'Paused until there is more battery or the computer is plugged in.',
    'motivo-AI_SUSPENDED_HEAT': 'Paused until the computer cools down.',
    excluir: 'Exclude from search by meaning',
    excluido: 'Excluded: it is not prepared, and anything already prepared has been deleted.',
    origenExacta: 'Exact',
    origenSignificado: 'By meaning',
    origenAmbas: 'Exact and by meaning'
  },
  pt: {
    titulo: 'Por Significado',
    sinModelo:
      'Para pesquisar por significado é preciso o modelo de IA, que se descarrega em Definições, em IA e Modelos.',
    listo: 'Preparado para pesquisar por significado:',
    fragmentos: 'fragmentos',
    documentos: 'documentos',
    de: 'de',
    costeDisco: 'Prepará-lo ocupa cerca de',
    costeTiempo: 'em disco e leva cerca de',
    costeProcesador: 'de processador',
    medido: 'medido neste computador',
    supuesto: 'calculado sobre o que costuma demorar',
    para: 'para',
    sinIndexar: 'Alguns ainda não estão indexados, por isso os seus fragmentos foram estimados.',
    noCabe:
      'Não cabe no espaço reservado para dados derivados. Pode ser aumentado em Definições, em Índice de Pesquisa.',
    preparar: 'Preparar',
    preparando: 'A preparar',
    enCola: 'documentos à espera',
    pausar: 'Pausar',
    reanudar: 'Retomar',
    cancelar: 'Cancelar',
    'motivo-USER_PAUSED': 'Em pausa.',
    'motivo-READER_ACTIVE': 'À espera de que deixe de virar páginas.',
    'motivo-AI_SUSPENDED_BATTERY':
      'Em pausa até haver mais bateria ou o computador ser ligado à corrente.',
    'motivo-AI_SUSPENDED_HEAT': 'Em pausa até o computador arrefecer.',
    excluir: 'Excluir da pesquisa por significado',
    excluido: 'Excluído: não é preparado, e o que já estivesse preparado foi apagado.',
    origenExacta: 'Exata',
    origenSignificado: 'Por significado',
    origenAmbas: 'Exata e por significado'
  }
};
