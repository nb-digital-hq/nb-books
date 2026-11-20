#!/usr/bin/env python3
"""
Genera los artboards de las pantallas de NB Books a partir de la carcasa aprobada
en Main.dc.html (Inicio).

Por qué un generador y no copias a mano: la continuidad entre pantallas es
exactamente lo que se está revisando, y diez copias de la misma barra lateral
escritas a mano se separan en el primer cambio. Aquí la hoja de estilos, los
tokens y los componentes salen del artboard de Inicio que ya se aprobó, y la
barra lateral y la superior se construyen una sola vez.

Uso:
    python3 generar_pantallas.py                 # todas las pantallas
    python3 generar_pantallas.py Biblioteca      # solo las que se nombren
"""

import html
import pathlib
import re
import sys

BASE = pathlib.Path(__file__).resolve().parent
RAIZ = BASE.parents[2]

MAIN = (BASE / 'Main.dc.html').read_text()
ICONOS = dict(
    re.findall(r"\n  ([a-zA-Z]+):\s*\n?\s*'([^']*)'", (RAIZ / 'src/lib/ui/iconos.ts').read_text())
)


def e(texto):
    return html.escape(texto, quote=False)


def ico(nombre, tam=24, estilo=''):
    return (
        f'<svg class="ico" viewBox="0 0 24 24" style="width: {tam}px; height: {tam}px; {estilo}">'
        f'{ICONOS[nombre]}</svg>'
    )


# ---------------------------------------------------------------------------
# Hoja de estilos: la de Inicio, más los componentes que Inicio no necesitaba.
# ---------------------------------------------------------------------------

COMPONENTES_CSS = '''
    /* --- Componentes de las demás pantallas: la misma familia que Inicio --- */
    .raiz { position: relative; display: grid; grid-template-columns: 256px minmax(0, 1fr); overflow: hidden; background: var(--sf); color: var(--on-sf); }
    .columna { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
    .lienzo { position: relative; flex: 1 1 auto; min-height: 0; margin: 0 16px 16px 0; border-radius: 28px; background: var(--sf-low); overflow: hidden; }
    .una-linea { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .dos-lineas { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
    .tres-lineas { display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }

    .boton-icono { display: flex; align-items: center; justify-content: center; width: 40px; height: 40px; border-radius: 20px; color: var(--on-sf-v); flex: 0 0 auto; }
    .boton-relleno, .boton-tonal, .boton-neutro {
      display: inline-flex; align-items: center; justify-content: center; gap: 8px;
      height: 40px; padding: 0 20px; border-radius: 20px; font-size: 14px; font-weight: 600; white-space: nowrap; box-sizing: border-box;
    }
    .boton-relleno { background: var(--p); color: var(--on-p); }
    .boton-tonal { background: var(--pc); color: var(--on-pc); }
    .boton-neutro { background: var(--sf-high); color: var(--on-sf); }
    .boton-enlace { display: inline-flex; align-items: center; gap: 4px; height: 32px; padding: 0 8px; border-radius: 16px; color: var(--p); font-size: 13px; font-weight: 600; }

    .chip { display: inline-flex; align-items: center; gap: 6px; height: 32px; padding: 0 12px; border-radius: 8px; background: var(--sf-c); color: var(--on-sf-v); font-size: 13px; font-weight: 500; white-space: nowrap; }
    .chip[data-sel='true'] { background: var(--pc); color: var(--on-pc); font-weight: 600; }
    .selector { display: inline-flex; align-items: center; gap: 8px; height: 40px; padding: 0 10px 0 14px; border-radius: 12px; background: var(--sf-c); color: var(--on-sf); font-size: 13px; font-weight: 500; white-space: nowrap; box-sizing: border-box; }
    .selector-prefijo { color: var(--on-sf-v); font-weight: 400; }
    .segmentado { display: inline-flex; gap: 2px; padding: 4px; border-radius: 14px; background: var(--sf-c); }
    .segmento { display: flex; align-items: center; justify-content: center; width: 40px; height: 32px; border-radius: 10px; color: var(--on-sf-v); }
    .segmento[data-sel='true'] { background: var(--sf-highest); color: var(--on-sf); }

    .panel { display: flex; flex-direction: column; min-height: 0; border-radius: 24px; background: var(--sf-c); overflow: hidden; box-sizing: border-box; }
    .panel-cabecera { display: flex; align-items: center; gap: 8px; height: 60px; padding: 0 12px 0 20px; flex: 0 0 auto; }
    .panel-titulo { margin: 0; font-size: 16px; font-weight: 600; color: var(--on-sf); }
    .etiqueta-sec { font-size: 12px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; color: var(--on-sf-v); }

    .campo { display: flex; align-items: center; gap: 12px; height: 52px; padding: 0 16px; border-radius: 14px; background: var(--sf-high); color: var(--on-sf-v); font-size: 15px; box-sizing: border-box; }
    .interruptor { position: relative; width: 52px; height: 32px; border-radius: 16px; background: var(--sf-highest); flex: 0 0 auto; }
    .interruptor::after { content: ''; position: absolute; left: 8px; top: 8px; width: 16px; height: 16px; border-radius: 8px; background: var(--outline); }
    .interruptor[data-on='true'] { background: var(--p); }
    .interruptor[data-on='true']::after { left: 24px; top: 4px; width: 24px; height: 24px; border-radius: 12px; background: var(--on-p); }

    .menu { position: absolute; z-index: 5; display: flex; flex-direction: column; border-radius: 20px; background: var(--sf-high); box-shadow: var(--elev-5); overflow: hidden; box-sizing: border-box; }
    .menu-item { display: flex; align-items: center; gap: 14px; min-height: 48px; padding: 0 16px; font-size: 14px; color: var(--on-sf); box-sizing: border-box; }

    .estado { display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 10px; border-radius: 13px; font-size: 12px; font-weight: 600; white-space: nowrap; }
    .estado[data-e='listo'] { background: var(--menta-c); color: var(--menta-on); }
    .estado[data-e='degradado'] { background: var(--melocoton-c); color: var(--melocoton-on); }
    .estado[data-e='fallo'] { background: var(--rosa-c); color: var(--rosa-on); }
    .estado[data-e='trabajando'] { background: var(--cielo-c); color: var(--cielo-on); }
    .punto { width: 6px; height: 6px; border-radius: 3px; background: currentColor; flex: 0 0 auto; }
'''

HELMET = re.search(r'<helmet>.*?</helmet>', MAIN, re.S).group(0)
_cierre = HELMET.rindex('</style>')
HELMET = HELMET[:_cierre] + COMPONENTES_CSS + '  ' + HELMET[_cierre:]


# ---------------------------------------------------------------------------
# Portadas: el tono sale del título, como en la aplicación.
# ---------------------------------------------------------------------------

TONOS = {
    'lavanda': ('#2f2c52', '#46426a', '#c6c0ff', '#f0ebff', '#c1bcec'),
    'cielo': ('#07334e', '#254a66', '#98ccf9', '#e1efff', '#a1c5e6'),
    'menta': ('#013825', '#1f4f3a', '#8ed5b1', '#c5fadc', '#99ccb0'),
    'melocoton': ('#4c2707', '#673d1b', '#ffb783', '#ffeadd', '#f1b489'),
    'rosa': ('#4b2339', '#653950', '#fcb0d5', '#ffe8f0', '#ebb0cc'),
    'limon': ('#393000', '#514711', '#d9c76f', '#feeea8', '#d0c17f'),
}


def tono_de(titulo):
    return list(TONOS)[sum(ord(c) for c in titulo) % len(TONOS)]


def portada(titulo, autor='', ancho=None, tam=15, estilo='', dentro='', relleno=None, con_pie=True):
    fondo, alto, orn, tit, aut = TONOS[tono_de(titulo)]
    medida = f'width: {ancho}px;' if ancho else 'width: 100%;'
    padding = f'padding: {relleno}px;' if relleno is not None else ''
    pie = (
        f'<div class="portada-pie"><span class="portada-filete"></span>'
        f'<span class="portada-autor">{e(autor)}</span></div>'
        if con_pie and autor
        else ''
    )
    return (
        f'<div class="portada" style="{medida} {padding} --fondo: {fondo}; --alto: {alto}; --orn: {orn}; '
        f'--tit: {tit}; --aut: {aut}; {estilo}">{dentro}'
        f'<div class="portada-titulo" style="font-size: {tam}px;">{e(titulo)}</div>{pie}</div>'
    )


# ---------------------------------------------------------------------------
# Barra lateral
# ---------------------------------------------------------------------------

EN_LINEA = (
    '<span class="destino-extra" style="display: flex; align-items: center; gap: 6px;">'
    '<span style="width: 6px; height: 6px; border-radius: 3px; background: var(--menta);"></span>En línea</span>'
)


def destino(icono, nombre, activo, extra=''):
    return (
        f'    <div class="destino" data-activo="{"true" if activo else "false"}">\n'
        f'      {ico(icono)}\n      <span>{nombre}</span>{extra}\n    </div>'
    )


TARJETA_LEYENDO = '''    <div style="display: flex; align-items: center; gap: 12px; padding: 12px; border-radius: 20px; background: var(--sf-c);">
      ''' + portada('Cien Años de Soledad', ancho=34, tam=1, relleno=0, con_pie=False, estilo='border-radius: 6px; box-shadow: none; color: transparent;') + '''
      <div style="display: flex; flex-direction: column; gap: 5px; min-width: 0; flex: 1 1 auto;">
        <span style="font-size: 12px; color: var(--on-sf-v);">Leyendo</span>
        <span class="serif una-linea" style="font-size: 14px; font-weight: 600;">Cien Años de Soledad</span>
        <div class="progreso"><span style="width: 38%;"></span></div>
      </div>
    </div>'''

TARJETA_INDICE = '''    <div style="display: flex; flex-direction: column; gap: 12px; padding: 16px; border-radius: 20px; background: var(--sf-c);">
      <div style="display: flex; align-items: center; gap: 10px; font-size: 13px; font-weight: 600; color: var(--on-sf);">
        ''' + ico('progreso', 20, 'color: var(--p);') + '''
        <span>Índice de Búsqueda</span>
        <span style="margin-left: auto; font-weight: 600; color: var(--p);">14 %</span>
      </div>
      <div class="progreso" style="height: 6px; border-radius: 3px;"><span style="width: 14%; border-radius: 3px;"></span></div>
      <span style="font-size: 12px; color: var(--on-sf-v);">20.687 de 151.411 obras</span>
    </div>'''


def barra_lateral(activo, lector=False, leyendo=True):
    destinos = [
        destino('inicio', 'Inicio', activo == 'inicio'),
        destino('biblioteca', 'Biblioteca', activo == 'biblioteca', '<span class="destino-extra">151 mil</span>'),
    ]
    if lector:
        destinos.append(destino('lector', 'Lector', activo == 'lector'))
    destinos += [
        destino('estudio', 'Estudio', activo == 'estudio'),
        destino('descubrir', 'Descubrir', activo == 'descubrir', EN_LINEA),
    ]
    tarjetas = ([TARJETA_LEYENDO] if leyendo else []) + [TARJETA_INDICE]
    return f'''  <nav style="display: flex; flex-direction: column; gap: 2px; height: 100%; padding: 0 12px 16px; box-sizing: border-box;">
    <div style="display: flex; justify-content: center; padding: 28px 0 28px;">
      <div style="display: flex; align-items: center; justify-content: center; width: 120px; height: 120px; border-radius: 60px; background: var(--p); box-shadow: var(--elev-3);">
        <div class="marca-silueta"></div>
      </div>
    </div>
{chr(10).join(destinos)}

    <div style="flex: 1 1 auto;"></div>

    <div style="display: flex; flex-direction: column; gap: 12px; margin-bottom: 12px;">
{chr(10).join(tarjetas)}
    </div>

{destino('ajustes', 'Ajustes', activo == 'ajustes')}
  </nav>'''


# ---------------------------------------------------------------------------
# Barra superior
# ---------------------------------------------------------------------------

def boton_de_menu(icono, texto, abierto=False, principal=False):
    if principal:
        fondo, color = ('var(--p)', 'var(--on-p)') if abierto else ('var(--pc)', 'var(--on-pc)')
    else:
        fondo, color = ('var(--pc)', 'var(--on-pc)') if abierto else ('var(--sf-high)', 'var(--on-sf)')
    sombra = 'box-shadow: var(--elev-1);' if principal else ''
    return (
        f'      <div style="display: flex; align-items: center; gap: 8px; height: 48px; padding: 0 14px 0 16px; '
        f'border-radius: 24px; background: {fondo}; color: {color}; font-size: 14px; font-weight: 600; '
        f'white-space: nowrap; {sombra} flex: 0 0 auto;">\n'
        f'        {ico(icono, 20)}\n        <span>{texto}</span>\n'
        f'        {ico("arriba" if abierto else "abajo", 18)}\n      </div>'
    )


def barra_superior(estanterias=False, abierto=None, consulta=None):
    if consulta:
        buscador = f'''      <div style="display: flex; align-items: center; gap: 16px; flex: 0 1 640px; min-width: 0; height: 56px; padding: 0 8px 0 20px; border-radius: 28px; background: var(--sf-highest); color: var(--on-sf); box-sizing: border-box;">
        {ico('buscar')}
        <span style="flex: 1 1 auto; min-width: 0; font-size: 16px; white-space: nowrap;">{e(consulta)}<span style="display: inline-block; width: 2px; height: 20px; margin-left: 2px; vertical-align: -4px; background: var(--p);"></span></span>
        <div class="boton-icono">{ico('cerrar', 20)}</div>
      </div>'''
    else:
        buscador = f'''      <div style="display: flex; align-items: center; gap: 16px; flex: 0 1 640px; min-width: 0; height: 56px; padding: 0 8px 0 20px; border-radius: 28px; background: var(--sf-high); color: var(--on-sf-v); box-sizing: border-box;">
        {ico('buscar')}
        <span style="flex: 1 1 auto; min-width: 0; font-size: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">Busca entre 151.411 obras, autores o una frase</span>
        <div class="boton-icono">{ico('filtrar', 20)}</div>
      </div>'''
    botones = []
    if estanterias:
        botones.append(boton_de_menu('estanteria', 'Estanterías', abierto == 'estanterias'))
    botones.append(boton_de_menu('carpeta', 'Colecciones', abierto == 'colecciones'))
    botones.append(boton_de_menu('importar', 'Importar', abierto == 'importar', principal=True))
    return f'''    <header style="display: flex; align-items: center; gap: 8px; height: 72px; padding: 0 16px 0 4px; box-sizing: border-box;">
      <div class="boton-icono" style="width: 48px; height: 48px; border-radius: 24px;">{ico('menu')}</div>
{buscador}
      <div style="flex: 1 1 auto;"></div>
{chr(10).join(botones)}
    </header>'''


def barra_superior_del_lector(titulo, subtitulo):
    return f'''    <header style="display: flex; align-items: center; gap: 8px; height: 72px; padding: 0 16px 0 4px; box-sizing: border-box;">
      <div class="boton-icono" style="width: 48px; height: 48px; border-radius: 24px;">{ico('menu')}</div>
      <div style="display: flex; flex-direction: column; gap: 2px; min-width: 0; padding-left: 8px;">
        <span class="serif una-linea" style="font-size: 18px; font-weight: 600;">{e(titulo)}</span>
        <span class="una-linea" style="font-size: 13px; color: var(--on-sf-v);">{e(subtitulo)}</span>
      </div>
      <div style="flex: 1 1 auto;"></div>
      <div class="boton-icono" style="width: 48px; height: 48px; border-radius: 24px;">{ico('buscar')}</div>
      <div class="boton-icono" style="width: 48px; height: 48px; border-radius: 24px;">{ico('cerrar')}</div>
    </header>'''


# ---------------------------------------------------------------------------
# Artboard
# ---------------------------------------------------------------------------

PLANTILLA = '''<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <script src="./support.js"></script>
</head>
<body>
<x-dc>
%HELMET%

<div class="nb raiz" data-tema="{{tema}}" style="width: %ANCHO%px; height: %ALTO%px;">
%NAV%
  <div class="columna">
%HEADER%
    <main class="lienzo">
%MAIN%
    </main>
  </div>
%CAPA%
</div>
</x-dc>

<script data-dc-script data-props='{"tema":{"editor":"enum","options":["grafito","gabinete","tinta","vigilia","musgo"],"default":"grafito","section":"Tema"}}'>
class Component extends DCLogic {
  renderVals() {
    return { tema: this.props.tema ?? 'grafito' };
  }
}
</script>
</body>
</html>
'''


def artboard(nav, header, main, capa='', ancho=1440, alto=900):
    return (
        PLANTILLA.replace('%HELMET%', HELMET)
        .replace('%ANCHO%', str(ancho))
        .replace('%ALTO%', str(alto))
        .replace('%NAV%', nav)
        .replace('%HEADER%', header)
        .replace('%MAIN%', main)
        .replace('%CAPA%', capa)
    )


# ---------------------------------------------------------------------------
# Biblioteca, con la ficha de la obra elegida
# ---------------------------------------------------------------------------

LIBROS = [
    ('Cien Años de Soledad', 'Gabriel García Márquez', {'progreso': 38, 'sel': True}),
    ('Rayuela', 'Julio Cortázar', {'progreso': 62}),
    ('Pedro Páramo', 'Juan Rulfo', {'progreso': 87}),
    ('La Casa de los Espíritus', 'Isabel Allende', {}),
    ('El Túnel', 'Ernesto Sabato', {'no_disponible': True}),
    ('Boquitas Pintadas', 'Manuel Puig', {}),
    ('Los Ríos Profundos', 'José María Arguedas', {'copias': 2}),
    ('Terra Nostra', 'Carlos Fuentes', {}),
    ('Santa Evita', 'Tomás Eloy Martínez', {'progreso': 12}),
    ('Ficciones', 'Jorge Luis Borges', {'progreso': 45}),
    ('La Ciudad y los Perros', 'Mario Vargas Llosa', {'progreso': 18}),
    ('Los Detectives Salvajes', 'Roberto Bolaño', {'progreso': 73}),
    ('El Aleph', 'Jorge Luis Borges', {}),
    ('Aura', 'Carlos Fuentes', {}),
    ('Crónica de una Muerte Anunciada', 'Gabriel García Márquez', {}),
]


def insignia(icono, texto):
    return (
        f'<span style="position: absolute; left: 8px; bottom: 8px; z-index: 1; display: flex; align-items: center; gap: 4px; '
        f'height: 22px; padding: 0 8px; border-radius: 6px; background: color-mix(in srgb, var(--sf) 82%, transparent); '
        f'color: var(--on-sf); font-size: 11px; font-weight: 600; white-space: nowrap;">{ico(icono, 13)}{e(texto)}</span>'
    )


def celda_de_libro(titulo, autor, datos):
    dentro = ''
    if datos.get('no_disponible'):
        dentro = insignia('noDisponible', 'No disponible')
    elif datos.get('copias'):
        dentro = insignia('duplicado', f'{datos["copias"]} copias')
    anillo = 'outline: 3px solid var(--p); outline-offset: 3px;' if datos.get('sel') else ''
    atenuado = 'opacity: 0.55;' if datos.get('no_disponible') else ''
    progreso = (
        f'<div class="progreso" style="margin-top: 2px;"><span style="width: {datos["progreso"]}%;"></span></div>'
        if datos.get('progreso')
        else '<div style="height: 6px;"></div>'
    )
    return f'''          <div style="display: flex; flex-direction: column; gap: 6px; min-width: 0;">
            {portada(titulo, autor, tam=14, relleno=12, dentro=dentro, estilo=anillo + atenuado, con_pie=False)}
            <span class="una-linea" style="margin-top: 6px; font-size: 14px; font-weight: 600;">{e(titulo)}</span>
            <span class="una-linea" style="font-size: 12px; color: var(--on-sf-v);">{e(autor)}</span>
            {progreso}
          </div>'''


def pantalla_biblioteca():
    chips = ''.join(
        f'<span class="chip" data-sel="{"true" if i == 0 else "false"}">{ico("comprobado", 16) if i == 0 else ""}{t}</span>'
        for i, t in enumerate(['Todos', 'Empezados', 'Duplicados', 'No disponibles'])
    )
    celdas = '\n'.join(celda_de_libro(t, a, d) for t, a, d in LIBROS)
    paginas = ''.join(
        f'<span style="display: flex; align-items: center; justify-content: center; min-width: 36px; height: 36px; padding: 0 8px; box-sizing: border-box; border-radius: 18px; font-size: 13px; font-weight: 600; {estilo}">{n}</span>'
        for n, estilo in [
            ('1', 'background: var(--pc); color: var(--on-pc);'),
            ('2', 'color: var(--on-sf-v);'),
            ('3', 'color: var(--on-sf-v);'),
            ('…', 'color: var(--on-sf-v);'),
            ('10.094', 'color: var(--on-sf-v);'),
        ]
    )
    ficha = f'''        <aside class="panel" style="width: 360px; flex: 0 0 360px; margin: 16px 16px 16px 0;">
          <div class="panel-cabecera">
            <h2 class="panel-titulo">Ficha</h2>
            <div style="flex: 1 1 auto;"></div>
            <div class="boton-icono">{ico('cerrar', 20)}</div>
          </div>
          <div style="display: flex; flex-direction: column; gap: 20px; padding: 0 20px 20px; min-height: 0; overflow: hidden;">
            <div style="display: flex; gap: 16px; align-items: flex-start;">
              {portada('Cien Años de Soledad', 'García Márquez', ancho=116, tam=15, relleno=12, con_pie=False, estilo='box-shadow: var(--elev-3);')}
              <div style="display: flex; flex-direction: column; gap: 8px; min-width: 0;">
                <h3 class="serif" style="margin: 0; font-size: 22px; font-weight: 600; line-height: 1.15; text-wrap: balance;">Cien Años de Soledad</h3>
                <span style="font-size: 14px; color: var(--on-sf-v);">Gabriel García Márquez</span>
                <span style="align-self: flex-start; display: flex; align-items: center; height: 24px; padding: 0 8px; border-radius: 6px; background: var(--melocoton-c); color: var(--melocoton-on); font-size: 12px; font-weight: 600;">Realismo Mágico</span>
              </div>
            </div>

            <div style="display: flex; gap: 8px;">
              <span class="boton-relleno" style="flex: 1 1 auto; height: 48px; border-radius: 24px;">{ico('reproducir', 20, 'fill: currentColor;')}Continuar</span>
              <span class="boton-neutro" style="width: 48px; height: 48px; padding: 0; border-radius: 24px;">{ico('mas', 22)}</span>
            </div>

            <div style="display: flex; flex-direction: column; gap: 8px;">
              <div style="display: flex; justify-content: space-between; font-size: 13px;"><span style="color: var(--on-sf-v);">Progreso</span><span style="font-weight: 600;">38 %</span></div>
              <div class="progreso" style="height: 6px; border-radius: 3px;"><span style="width: 38%; border-radius: 3px;"></span></div>
            </div>

            <dl style="display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 10px 16px; margin: 0; font-size: 13px;">
              <dt style="color: var(--on-sf-v);">Serie</dt><dd style="margin: 0;">—</dd>
              <dt style="color: var(--on-sf-v);">Añadido</dt><dd style="margin: 0;">12 de septiembre de 2026</dd>
              <dt style="color: var(--on-sf-v);">Copias</dt><dd style="margin: 0; font-weight: 600; color: var(--p);">2 en la biblioteca</dd>
            </dl>

            <div style="display: flex; flex-direction: column; gap: 10px; padding: 16px; border-radius: 16px; background: var(--sf-high);">
              <span class="boton-neutro" style="align-self: flex-start; background: var(--sf-highest);">{ico('descubrir', 18)}Completar Desde un Catálogo</span>
              <span style="font-size: 12px; line-height: 1.45; color: var(--on-sf-v);">Sale a un catálogo abierto con el título y el autor; lo que llegue no pisa lo tuyo.</span>
            </div>

            <div style="display: flex; flex-direction: column; gap: 10px;">
              <span class="etiqueta-sec">Propuestas Sin Aplicar</span>
              <div style="display: flex; align-items: center; gap: 12px; padding: 12px 12px 12px 16px; border-radius: 16px; background: var(--sf-high);">
                <div style="display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1 1 auto;">
                  <span style="font-size: 12px; color: var(--on-sf-v);">Editorial</span>
                  <span class="una-linea" style="font-size: 14px; font-weight: 600;">Sudamericana</span>
                </div>
                <span class="boton-tonal" style="height: 36px; padding: 0 14px;">Aceptar</span>
                <span class="boton-enlace" style="color: var(--on-sf-v);">Rechazar</span>
              </div>
            </div>
          </div>
        </aside>'''
    return f'''      <div style="display: flex; height: 100%;">
        <div style="display: flex; flex-direction: column; gap: 20px; flex: 1 1 auto; min-width: 0; padding: 20px 20px 12px 32px;">
          <div style="display: flex; align-items: center; gap: 8px;">
            {chips}
            <div style="flex: 1 1 auto;"></div>
            <span class="selector"><span class="selector-prefijo">Ordenar por</span> Añadido {ico('abajo', 18)}</span>
            <span class="segmentado">
              <span class="segmento" data-sel="true">{ico('cuadricula', 20)}</span>
              <span class="segmento">{ico('lista', 20)}</span>
              <span class="segmento">{ico('mosaico', 20)}</span>
            </span>
          </div>

          <div style="position: relative; flex: 1 1 auto; min-height: 0; overflow: hidden;">
            <div style="display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 28px 20px; padding: 4px;">
{celdas}
            </div>
            <div style="position: absolute; left: 0; right: 0; bottom: 0; height: 80px; pointer-events: none; background: linear-gradient(180deg, transparent, var(--sf-low));"></div>
          </div>

          <div style="display: flex; align-items: center; gap: 8px; height: 44px;">
            <span style="font-size: 13px; color: var(--on-sf-v);">1–15 de 151.411 documentos</span>
            <div style="flex: 1 1 auto;"></div>
            <span class="boton-flecha" style="opacity: 0.38;">{ico('izquierda')}</span>
            {paginas}
            <span class="boton-flecha">{ico('derecha')}</span>
          </div>
        </div>
{ficha}
      </div>'''


# ---------------------------------------------------------------------------
# Lector, con las anotaciones abiertas
# ---------------------------------------------------------------------------

def tarjeta_de_anotacion(icono, clase, color, pagina, cita, cuerpo=''):
    nota = f'<span class="dos-lineas" style="font-size: 13px; line-height: 1.45; color: var(--on-sf-v);">{e(cuerpo)}</span>' if cuerpo else ''
    return f'''            <div style="display: flex; flex-direction: column; gap: 8px; padding: 14px 14px 8px 16px; border-radius: 16px; background: var(--sf-high);">
              <div style="display: flex; align-items: center; gap: 8px; font-size: 12px; font-weight: 600;">
                {ico(icono, 16, f'color: {color};')}<span style="color: {color};">{clase}</span>
                <span style="margin-left: auto; font-weight: 500; color: var(--on-sf-v);">{pagina}</span>
              </div>
              <span class="serif dos-lineas" style="font-size: 15px; font-style: italic; line-height: 1.45;">«{e(cita)}»</span>
              {nota}
              <span class="boton-enlace" style="align-self: flex-start; margin-left: -8px;">Ir al Texto</span>
            </div>'''


def pantalla_lector():
    herramientas = ''.join(
        f'<span class="boton-icono" style="width: 44px; height: 44px; border-radius: 22px; {"background: var(--pc); color: var(--on-pc);" if activo else ""}">{ico(nombre, 22)}</span>'
        for nombre, activo in [
            ('indice', False), ('anotaciones', True), ('diccionario', False), ('voz', False),
            ('modoLectura', False), ('tipografia', False), ('fondoSonoro', False),
            ('vistaDividida', False), ('inmersion', False),
        ]
    )
    parrafo = 'margin: 0 0 22px; font-size: 19px; line-height: 1.78; color: var(--on-sf); text-wrap: pretty;'
    anotaciones = '\n'.join([
        tarjeta_de_anotacion('nota', 'Nota', 'var(--p)', 'Pág. 2', 'no ha mucho tiempo que vivía un hidalgo de los de lanza en astillero',
                             'La parodia funciona porque el hidalgo es contemporáneo del lector, no un caballero de otro siglo.'),
        tarjeta_de_anotacion('resaltar', 'Resaltado', 'var(--limon)', 'Pág. 3', 'gran madrugador y amigo de la caza'),
        tarjeta_de_anotacion('subrayar', 'Subrayado', 'var(--cielo)', 'Pág. 3', 'se deja entender que se llamaba Quijana'),
        tarjeta_de_anotacion('marcador', 'Marcador', 'var(--rosa)', 'Pág. 5', 'Que trata de la primera salida que de su tierra hizo el ingenioso don Quijote'),
    ])
    chips = ''.join(
        f'<span class="chip" data-sel="{"true" if i == 0 else "false"}" style="height: 30px; background: {"var(--pc)" if i == 0 else "var(--sf-high)"};">{t}</span>'
        for i, t in enumerate(['Todas', 'Notas', 'Resaltados', 'Marcadores'])
    )
    return f'''      <div style="display: flex; height: 100%;">
        <div style="position: relative; flex: 1 1 auto; min-width: 0; overflow: hidden;">
          <article style="max-width: 640px; margin: 0 auto; padding: 44px 40px 0;">
            <div style="font-size: 12px; font-weight: 600; letter-spacing: 0.12em; text-transform: uppercase; color: var(--p);">Primera Parte · Capítulo I</div>
            <h1 class="serif" style="margin: 12px 0 28px; font-size: 30px; font-weight: 600; line-height: 1.25; text-wrap: balance;">Que trata de la condición y ejercicio del famoso y valiente hidalgo don Quijote de la Mancha</h1>

            <p class="serif" style="position: relative; {parrafo}"><span style="position: absolute; left: -34px; top: 8px; color: var(--p);">{ico('nota', 18)}</span>En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha mucho tiempo que vivía un hidalgo de los de lanza en astillero, adarga antigua, rocín flaco y <span style="padding: 1px 0; border-radius: 3px; background: color-mix(in srgb, var(--p) 34%, transparent);">galgo corredor</span>. Una olla de algo más vaca que carnero, salpicón las más noches, duelos y quebrantos los sábados, lantejas los viernes, algún palomino de añadidura los domingos, consumían las tres partes de su hacienda.</p>

            <p class="serif" style="{parrafo}">Frisaba la edad de nuestro hidalgo con los cincuenta años. Era de complexión recia, seco de carnes, enjuto de rostro, <span style="padding: 1px 0; border-radius: 3px; background: color-mix(in srgb, var(--limon) 30%, transparent);">gran madrugador y amigo de la caza</span>. Quieren decir que tenía el sobrenombre de Quijada, o Quesada, aunque por conjeturas verisímiles <span style="text-decoration: underline; text-decoration-color: var(--cielo); text-decoration-thickness: 2px; text-underline-offset: 5px;">se deja entender que se llamaba Quijana</span>.</p>

            <p class="serif" style="{parrafo}">Es, pues, de saber que este sobredicho hidalgo, los ratos que estaba ocioso, que eran los más del año, se daba a leer libros de caballerías, con tanta afición y gusto, que olvidó casi de todo punto el ejercicio de la caza y aun la administración de su hacienda.</p>
          </article>

          <div style="position: absolute; left: 0; right: 0; bottom: 0; height: 240px; pointer-events: none; background: linear-gradient(180deg, transparent, var(--sf-low) 52%);"></div>

          <div style="position: absolute; left: 50%; bottom: 44px; transform: translateX(-50%); display: flex; gap: 2px; padding: 6px; border-radius: 30px; background: var(--sf-high); box-shadow: var(--elev-3);">
            {herramientas}
          </div>

          <div style="position: absolute; left: 32px; right: 32px; bottom: 16px; display: flex; align-items: center; gap: 14px; font-size: 12px; color: var(--on-sf-v);">
            <span style="white-space: nowrap;">Capítulo I de LII</span>
            <div class="progreso" style="flex: 1 1 auto; height: 4px;"><span style="width: 1%;"></span></div>
            <span style="white-space: nowrap;">Página 3 de 1.312</span>
          </div>
        </div>

        <aside class="panel" style="width: 360px; flex: 0 0 360px; margin: 16px 16px 16px 0;">
          <div class="panel-cabecera">
            <h2 class="panel-titulo">Anotaciones</h2>
            <span style="font-size: 13px; color: var(--on-sf-v);">12</span>
            <div style="flex: 1 1 auto;"></div>
            <div class="boton-icono">{ico('cerrar', 20)}</div>
          </div>
          <div style="display: flex; flex-wrap: wrap; gap: 6px; padding: 0 20px 14px;">{chips}</div>
          <div style="display: flex; flex-direction: column; gap: 10px; flex: 1 1 auto; min-height: 0; padding: 0 16px; overflow: hidden;">
{anotaciones}
          </div>
          <div style="display: flex; flex-direction: column; gap: 10px; padding: 16px 20px 20px;">
            <span class="selector" style="justify-content: space-between; background: var(--sf-high);"><span><span class="selector-prefijo">Citas en</span> APA 7</span>{ico('abajo', 18)}</span>
            <span class="boton-tonal" style="height: 44px; border-radius: 22px;">{ico('exportar', 20)}Llevarme Estas Anotaciones</span>
          </div>
        </aside>
      </div>'''


# ---------------------------------------------------------------------------
# Estudio: la pestaña de notas, con enlaces, pasajes y quién apunta aquí
# ---------------------------------------------------------------------------

def enlace_de_nota(titulo):
    return (
        f'<span style="display: inline-flex; align-items: center; gap: 4px; padding: 0 6px; border-radius: 6px; '
        f'background: color-mix(in srgb, var(--p) 16%, transparent); color: var(--p); font-weight: 500; white-space: nowrap;">'
        f'{ico("enlace", 14)}{e(titulo)}</span>'
    )


def pestana_de_estudio(icono, nombre, cuenta, sel):
    color = 'var(--p)' if sel else 'var(--on-sf-v)'
    peso = '600' if sel else '500'
    fondo_cuenta = 'var(--pc)' if sel else 'var(--sf-c)'
    color_cuenta = 'var(--on-pc)' if sel else 'var(--on-sf-v)'
    indicador = (
        '<span style="position: absolute; left: 14px; right: 14px; bottom: 0; height: 3px; '
        'border-radius: 3px 3px 0 0; background: var(--p);"></span>'
        if sel
        else ''
    )
    return (
        f'<div style="position: relative; display: flex; align-items: center; gap: 8px; height: 56px; padding: 0 18px; '
        f'color: {color}; font-size: 14px; font-weight: {peso};">{ico(icono, 20)}<span>{nombre}</span>'
        f'<span style="display: flex; align-items: center; height: 20px; padding: 0 7px; border-radius: 10px; '
        f'background: {fondo_cuenta}; color: {color_cuenta}; font-size: 12px; font-weight: 600;">{cuenta}</span>'
        f'{indicador}</div>'
    )


def fila_de_nota(titulo, extracto, cuando, enlaces, sel):
    fondo = 'background: var(--sf-high);' if sel else ''
    return (
        f'            <div style="display: flex; flex-direction: column; gap: 6px; padding: 14px 16px; border-radius: 16px; {fondo}">\n'
        f'              <span class="serif una-linea" style="font-size: 15px; font-weight: 600;">{e(titulo)}</span>\n'
        f'              <span class="dos-lineas" style="font-size: 13px; line-height: 1.45; color: var(--on-sf-v);">{e(extracto)}</span>\n'
        f'              <span style="display: flex; align-items: center; gap: 10px; font-size: 12px; color: var(--on-sf-v);">'
        f'{e(cuando)}<span style="display: flex; align-items: center; gap: 4px;">{ico("enlace", 13)}{enlaces}</span></span>\n'
        f'            </div>'
    )


def pasaje_citado(cita, donde):
    return (
        f'                <div style="display: flex; gap: 12px; align-items: flex-start; padding: 14px 8px 14px 16px; border-radius: 16px; background: var(--sf-c);">\n'
        f'                  <div style="display: flex; flex-direction: column; gap: 6px; min-width: 0; flex: 1 1 auto;">\n'
        f'                    <span class="serif" style="font-size: 16px; font-style: italic; line-height: 1.5;">«{e(cita)}»</span>\n'
        f'                    <span style="font-size: 12px; color: var(--on-sf-v);">{e(donde)}</span>\n'
        f'                  </div>\n'
        f'                  <span class="boton-icono">{ico("cerrar", 18)}</span>\n'
        f'                </div>'
    )


def retroenlace_de_nota(titulo, antes, despues):
    return (
        f'            <div style="display: flex; flex-direction: column; gap: 6px; padding: 14px 16px; border-radius: 16px; background: var(--sf-high);">\n'
        f'              <span class="serif una-linea" style="font-size: 14px; font-weight: 600;">{e(titulo)}</span>\n'
        f'              <span style="font-size: 13px; line-height: 1.6; color: var(--on-sf-v);">…{e(antes)} '
        f'{enlace_de_nota("La parodia como método en el Quijote")} {e(despues)}…</span>\n'
        f'            </div>'
    )


def pantalla_estudio():
    pestanas = ''.join(
        pestana_de_estudio(*datos)
        for datos in [
            ('nota', 'Notas', '128', True),
            ('anotaciones', 'Anotaciones', '2.311', False),
            ('vocabulario', 'Vocabulario', '406', False),
            ('cuadricula', 'Tableros', '7', False),
        ]
    )
    notas = '\n'.join(
        fila_de_nota(*datos)
        for datos in [
            ('La parodia como método en el Quijote', 'Cervantes no se burla de los libros de caballerías desde fuera: los imita con tanta exactitud que la distancia aparece sola.', 'Hace 2 días', 3, True),
            ('Lectores dentro de la novela', 'El cura, el barbero y el ventero leen, y cada uno lee distinto: la novela está llena de lectores.', 'Hace 4 días', 2, False),
            ('El narrador poco fiable', 'Cide Hamete Benengeli traduce, resume y a veces miente. ¿Quién cuenta de verdad la historia?', 'La semana pasada', 1, False),
            ('Metaficción antes de la palabra', 'La segunda parte sabe que existe la primera y la comenta. Eso ocurre en 1615.', 'La semana pasada', 2, False),
            ('Cervantes y los libros de caballerías', 'Amadís, Tirante el Blanco y el escrutinio de la biblioteca: qué se salva y por qué.', 'Hace 2 semanas', 4, False),
            ('Preguntas para la tesis', 'Tres preguntas abiertas antes de la reunión del jueves con la directora.', 'Hace 3 semanas', 5, False),
        ]
    )
    return f'''      <div style="display: flex; flex-direction: column; height: 100%;">
        <div style="display: flex; align-items: flex-end; gap: 4px; padding: 8px 24px 0;">
          {pestanas}
        </div>

        <div style="display: flex; gap: 16px; flex: 1 1 auto; min-height: 0; padding: 16px;">
          <div style="display: flex; flex-direction: column; gap: 12px; width: 300px; flex: 0 0 300px; min-height: 0;">
            <div style="display: flex; align-items: center; gap: 8px; padding: 0 4px 0 16px;">
              <span style="font-size: 13px; color: var(--on-sf-v);">128 notas</span>
              <div style="flex: 1 1 auto;"></div>
              <span class="boton-tonal" style="height: 40px; padding: 0 16px;">{ico('mas', 18)}Nota Nueva</span>
            </div>
            <div style="display: flex; flex-direction: column; gap: 2px; min-height: 0; overflow: hidden;">
{notas}
            </div>
          </div>

          <div style="display: flex; flex-direction: column; gap: 22px; flex: 1 1 auto; min-width: 0; padding: 24px 36px; border-radius: 24px; background: var(--sf); overflow: hidden;">
            <div style="display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--on-sf-v);">
              <span style="white-space: nowrap;">Editada hace 2 días</span>
              <span class="boton-enlace" style="white-space: nowrap;">Versiones</span>
              <div style="flex: 1 1 auto;"></div>
              <span class="boton-enlace" style="color: var(--on-sf-v);">Borrar</span>
              <span class="boton-relleno">Guardar</span>
            </div>
            <h1 class="serif" style="margin: 0; font-size: 34px; font-weight: 600; line-height: 1.15; letter-spacing: -0.015em;">La parodia como método en el Quijote</h1>
            <div style="display: flex; flex-direction: column; gap: 14px; font-size: 16px; line-height: 1.8; color: var(--on-sf);">
              <p style="margin: 0;">Cervantes no se burla de los libros de caballerías desde fuera: los imita con tanta exactitud que la distancia aparece sola. El hidalgo es contemporáneo del lector, y por eso {enlace_de_nota('Lectores dentro de la novela')} funciona como espejo.</p>
              <p style="margin: 0;">La hipótesis para el capítulo dos es que la parodia es un método de lectura y no solo un género. Ver {enlace_de_nota('El narrador poco fiable')} y {enlace_de_nota('Metaficción antes de la palabra')}.</p>
            </div>
            <div style="display: flex; flex-direction: column; gap: 10px;">
              <span class="etiqueta-sec">Pasajes que Cita</span>
{pasaje_citado('no ha mucho tiempo que vivía un hidalgo de los de lanza en astillero', 'Don Quijote de la Mancha · Capítulo I · Pág. 2')}
{pasaje_citado('se daba a leer libros de caballerías, con tanta afición y gusto', 'Don Quijote de la Mancha · Capítulo I · Pág. 4')}
            </div>
            <span style="font-size: 12px; color: var(--on-sf-v);">Escribe [[Título]] para enlazar con otra nota.</span>
          </div>

          <aside class="panel" style="width: 300px; flex: 0 0 300px;">
            <div class="panel-cabecera">
              {ico('retroenlace', 20, 'color: var(--p);')}
              <h2 class="panel-titulo">Apuntan Aquí</h2>
              <span style="font-size: 13px; color: var(--on-sf-v);">2</span>
            </div>
            <div style="display: flex; flex-direction: column; gap: 10px; padding: 0 16px 16px;">
{retroenlace_de_nota('Preguntas para la tesis', 'antes del jueves, revisar si', 'aguanta el capítulo dos entero')}
{retroenlace_de_nota('Cervantes y los libros de caballerías', 'el escrutinio de la biblioteca es la prueba de que', 'no es una exageración')}
            </div>
          </aside>
        </div>
      </div>'''


# ---------------------------------------------------------------------------
# Descubrir: los catálogos y la fuente abierta
# ---------------------------------------------------------------------------

def fila_de_catalogo(nombre, anfitrion, sel):
    fondo = 'background: var(--sf-high);' if sel else ''
    fondo_icono = 'var(--pc)' if sel else 'var(--sf-high)'
    color_icono = 'var(--on-pc)' if sel else 'var(--on-sf-v)'
    return (
        f'            <div style="display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: 16px; {fondo}">\n'
        f'              <span style="display: flex; align-items: center; justify-content: center; width: 40px; height: 40px; border-radius: 12px; '
        f'background: {fondo_icono}; color: {color_icono}; flex: 0 0 auto;">{ico("descubrir", 20)}</span>\n'
        f'              <div style="display: flex; flex-direction: column; gap: 2px; min-width: 0;">\n'
        f'                <span class="una-linea" style="font-size: 14px; font-weight: 600;">{e(nombre)}</span>\n'
        f'                <span class="una-linea" style="font-size: 12px; color: var(--on-sf-v);">{e(anfitrion)}</span>\n'
        f'              </div>\n'
        f'            </div>'
    )


def entrada_de_catalogo(titulo, autor, resumen, estado):
    if estado == 'ya':
        accion = f'<span class="estado" data-e="listo">{ico("comprobado", 14)}Ya estaba en tu biblioteca</span>'
    elif estado == 'bajando':
        accion = (
            '<div style="display: flex; align-items: center; gap: 10px; width: 200px;">'
            '<span style="font-size: 12px; font-weight: 600; color: var(--p);">Descargando</span>'
            '<div class="progreso" style="flex: 1 1 auto;"><span style="width: 64%;"></span></div></div>'
        )
    else:
        accion = f'<span class="boton-tonal" style="height: 36px; padding: 0 16px;">{ico("importar", 18)}Descargar</span>'
    return (
        f'              <div style="display: flex; gap: 16px; padding: 16px; border-radius: 20px; background: var(--sf-c);">\n'
        f'                {portada(titulo, autor, ancho=92, tam=12, relleno=10, con_pie=False)}\n'
        f'                <div style="display: flex; flex-direction: column; gap: 6px; min-width: 0; flex: 1 1 auto;">\n'
        f'                  <span class="serif una-linea" style="font-size: 17px; font-weight: 600;">{e(titulo)}</span>\n'
        f'                  <span class="una-linea" style="font-size: 13px; color: var(--on-sf-v);">{e(autor)}</span>\n'
        f'                  <span class="dos-lineas" style="font-size: 13px; line-height: 1.45; color: var(--on-sf-v);">{e(resumen)}</span>\n'
        f'                  <div style="flex: 1 1 auto;"></div>\n'
        f'                  <div style="display: flex; align-items: center;">{accion}</div>\n'
        f'                </div>\n'
        f'              </div>'
    )


def pantalla_descubrir():
    catalogos = '\n'.join(
        fila_de_catalogo(*datos)
        for datos in [
            ('Project Gutenberg', 'gutenberg.org', True),
            ('Mi biblioteca universitaria', 'biblioteca.ejemplo.edu', False),
            ('Club de lectura', 'club.ejemplo.org', False),
        ]
    )
    navegacion = ''.join(
        f'<span class="chip" style="height: 36px; padding: 0 14px; color: var(--on-sf);">{e(t)}{ico("derecha", 16)}</span>'
        for t in ['Más descargados', 'Por autor', 'Por materia', 'Teatro', 'Poesía']
    )
    entradas = '\n'.join(
        entrada_de_catalogo(*datos)
        for datos in [
            ('Don Quijote de la Mancha', 'Miguel de Cervantes', 'Un hidalgo que ha leído demasiado sale a corregir un mundo que no se parece a sus libros.', 'ya'),
            ('La Regenta', 'Leopoldo Alas «Clarín»', 'Vetusta, una ciudad que lo vigila todo, y una mujer que se ahoga dentro de ella.', 'descargar'),
            ('Niebla', 'Miguel de Unamuno', 'Augusto Pérez descubre que es un personaje y va a discutirlo con su autor.', 'bajando'),
            ('Fortunata y Jacinta', 'Benito Pérez Galdós', 'Dos mujeres, un mismo hombre y el Madrid entero de fondo.', 'descargar'),
            ('Pepita Jiménez', 'Juan Valera', 'Las cartas de un seminarista que no sabe que se está enamorando.', 'descargar'),
            ('Rimas y Leyendas', 'Gustavo Adolfo Bécquer', 'Poemas breves y relatos de misterio: el romanticismo en voz baja.', 'descargar'),
        ]
    )
    return f'''      <div style="display: flex; gap: 16px; height: 100%; padding: 16px; box-sizing: border-box;">
        <aside class="panel" style="width: 300px; flex: 0 0 300px; background: var(--sf);">
          <div class="panel-cabecera"><h2 class="panel-titulo">Catálogos</h2></div>
          <div style="display: flex; flex-direction: column; gap: 4px; padding: 0 12px;">
{catalogos}
          </div>
          <div style="flex: 1 1 auto;"></div>
          <div style="display: flex; flex-direction: column; gap: 8px; padding: 16px 20px 20px;">
            <span style="font-size: 12px; line-height: 1.5; color: var(--on-sf-v);">Ninguno viene puesto: los añades tú, y no se sale a ninguno hasta que lo autorizas.</span>
            <span class="boton-enlace" style="align-self: flex-start; margin-left: -8px;">{ico('mas', 16)}Añadir en Ajustes</span>
          </div>
        </aside>

        <div style="display: flex; flex-direction: column; gap: 18px; flex: 1 1 auto; min-width: 0; padding: 8px 16px 0 8px; overflow: hidden;">
          <div style="display: flex; align-items: center; gap: 12px;">
            <span class="boton-flecha">{ico('izquierda')}</span>
            <div style="display: flex; flex-direction: column; gap: 2px; min-width: 0;">
              <span style="font-size: 13px; color: var(--on-sf-v);">Project Gutenberg</span>
              <h1 style="margin: 0; font-size: 24px; font-weight: 600; letter-spacing: -0.01em;">Libros en Español</h1>
            </div>
            <div style="flex: 1 1 auto;"></div>
            <span class="estado" data-e="listo"><span class="punto"></span>En línea · autorizado</span>
          </div>
          <div style="display: flex; flex-wrap: wrap; gap: 8px;">{navegacion}</div>
          <div style="position: relative; flex: 1 1 auto; min-height: 0; overflow: hidden;">
            <div style="display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px;">
{entradas}
            </div>
            <div style="position: absolute; left: 0; right: 0; bottom: 0; height: 90px; pointer-events: none; background: linear-gradient(180deg, transparent, var(--sf-low));"></div>
          </div>
          <div style="display: flex; justify-content: center; padding-bottom: 16px;">
            <span class="boton-neutro">Página Siguiente{ico('derecha', 18)}</span>
          </div>
        </div>
      </div>'''


# ---------------------------------------------------------------------------
# Ajustes: la navegación propia de Ajustes, Apariencia y Acerca de
# ---------------------------------------------------------------------------

SECCIONES_DE_AJUSTES = [
    ('apariencia', 'Apariencia'),
    ('accesibilidad', 'Accesibilidad'),
    ('idioma', 'Idioma'),
    ('voz', 'Voz'),
    ('privacidad', 'Red y Privacidad'),
    ('descubrir', 'Catálogos'),
    ('diccionario', 'Diccionarios'),
    ('extensiones', 'Extensiones'),
    ('buscar', 'Índice de Búsqueda'),
    ('almacenamiento', 'Dónde Viven tus Libros'),
    ('informacion', 'Acerca de'),
]

# Los siete temas de la aplicación, con los valores que deriva su generador (src/paletas.css):
# nombre, semilla, primario, contenedor, fondo, lienzo, superficie alta y texto.
PALETAS_DE_TEMA = [
    ('Grafito', '#D97757', '#ffb59e', '#723521', '#131313', '#1b1b1b', '#2a2a2a', '#e2e2e2'),
    ('Gabinete', '#D9A441', '#efbf6d', '#5f4100', '#17130b', '#201b13', '#2f2921', '#ece1d4'),
    ('Papel', '#8C2A20', '#904a41', '#ffdad5', '#fff8f7', '#fff0ee', '#f7e4e1', '#231918'),
    ('Tinta', '#4EC9B0', '#84d6c2', '#005144', '#0e1513', '#171d1b', '#252b29', '#dee4e0'),
    ('Pergamino', '#7A4715', '#88511d', '#ffdcc2', '#fff8f5', '#fff1e9', '#f5e5db', '#221a14'),
    ('Vigilia', '#8FA9DD', '#abc7ff', '#294677', '#111318', '#1a1c20', '#282a2f', '#e2e2e9'),
    ('Musgo', '#93C3A5', '#91d5ad', '#035233', '#0f1511', '#171d19', '#262b27', '#dfe4dd'),
]

DESCRIPCIONES_DE_TEMA = {
    'Grafito': 'Grises y un acento',
    'Gabinete': 'La calma de una biblioteca',
    'Papel': 'Luz de día',
    'Tinta': 'Precisión sin adornos',
    'Pergamino': 'Lo copiado a mano',
    'Vigilia': 'La noche',
    'Musgo': 'Lo vegetal y lento',
}

# Los seis acentos que ofrece Ajustes, los de SettingsView.svelte.
ACENTOS = ['#d9a441', '#c8674a', '#7fa88b', '#8a9ac9', '#b07fb0', '#6f9ec9']


def navegacion_de_ajustes(activa):
    filas = []
    for icono, nombre in SECCIONES_DE_AJUSTES:
        sel = nombre == activa
        fondo = 'background: var(--pc); color: var(--on-pc); font-weight: 600;' if sel else 'color: var(--on-sf-v); font-weight: 500;'
        filas.append(
            f'            <div style="display: flex; align-items: center; gap: 14px; height: 48px; padding: 0 16px; '
            f'border-radius: 24px; font-size: 14px; {fondo}">{ico(icono, 22)}<span class="una-linea">{nombre}</span></div>'
        )
    return (
        '          <nav style="display: flex; flex-direction: column; gap: 2px; width: 280px; flex: 0 0 280px; padding: 24px 12px; box-sizing: border-box;">\n'
        '            <h1 class="serif" style="margin: 0 0 16px 16px; font-size: 26px; font-weight: 600;">Ajustes</h1>\n'
        + '\n'.join(filas)
        + '\n          </nav>'
    )


def vista_previa_de_tema(p, pc, sf, lienzo, alta, texto):
    return (
        f'<div style="display: grid; grid-template-columns: 34px minmax(0, 1fr); gap: 8px; height: 118px; padding: 10px; '
        f'border-radius: 14px; background: {sf}; box-sizing: border-box;">'
        f'<div style="display: flex; flex-direction: column; align-items: center; gap: 7px;">'
        f'<span style="width: 22px; height: 22px; border-radius: 11px; background: {p};"></span>'
        f'<span style="width: 26px; height: 10px; border-radius: 5px; background: {pc};"></span>'
        f'<span style="width: 26px; height: 10px; border-radius: 5px; background: {alta};"></span></div>'
        f'<div style="display: flex; flex-direction: column; gap: 7px; padding: 10px; border-radius: 10px; background: {lienzo};">'
        f'<span style="width: 60%; height: 8px; border-radius: 4px; background: {texto}; opacity: 0.85;"></span>'
        f'<span style="width: 88%; height: 6px; border-radius: 3px; background: {texto}; opacity: 0.35;"></span>'
        f'<span style="width: 72%; height: 6px; border-radius: 3px; background: {texto}; opacity: 0.35;"></span>'
        f'<span style="margin-top: auto; width: 48px; height: 14px; border-radius: 7px; background: {p};"></span></div></div>'
    )


def tarjeta_de_tema(nombre, semilla, p, pc, sf, lienzo, alta, texto, sel=False, subtitulo=None, previa=None):
    anillo = 'outline: 3px solid var(--p); outline-offset: 3px;' if sel else ''
    marca = (
        f'<span style="display: flex; align-items: center; justify-content: center; width: 26px; height: 26px; '
        f'border-radius: 13px; background: var(--p); color: var(--on-p);">{ico("comprobado", 16)}</span>'
        if sel
        else ''
    )
    return (
        f'              <div style="display: flex; flex-direction: column; gap: 10px; padding: 10px; border-radius: 20px; background: var(--sf-c); {anillo}">\n'
        f'                {previa or vista_previa_de_tema(p, pc, sf, lienzo, alta, texto)}\n'
        f'                <div style="display: flex; align-items: center; gap: 8px; padding: 0 6px 4px;">\n'
        f'                  <div style="display: flex; flex-direction: column; gap: 2px; min-width: 0;">\n'
        f'                    <span style="font-size: 14px; font-weight: 600;">{e(nombre)}</span>\n'
        f'                    <span style="font-size: 12px; color: var(--on-sf-v);">{e(subtitulo or "Semilla " + semilla)}</span>\n'
        f'                  </div>\n'
        f'                  <div style="flex: 1 1 auto;"></div>{marca}\n'
        f'                </div>\n'
        f'              </div>'
    )


def pantalla_ajustes_apariencia():
    tarjetas = [
        tarjeta_de_tema(*datos, sel=(i == 0), subtitulo=DESCRIPCIONES_DE_TEMA[datos[0]])
        for i, datos in enumerate(PALETAS_DE_TEMA)
    ]
    grafito = PALETAS_DE_TEMA[0]
    papel = next(d for d in PALETAS_DE_TEMA if d[0] == 'Papel')
    # Sistema no es un tema: es Grafito de noche y Papel de día, y la miniatura enseña las dos mitades.
    previa = (
        f'<div style="position: relative; height: 118px;">'
        f'<div style="position: absolute; inset: 0;">{vista_previa_de_tema(*grafito[2:])}</div>'
        f'<div style="position: absolute; inset: 0; clip-path: polygon(58% 0, 100% 0, 100% 100%, 42% 100%);">{vista_previa_de_tema(*papel[2:])}</div></div>'
    )
    tarjetas.append(tarjeta_de_tema('Sistema', '', *grafito[2:], subtitulo='Sigue al escritorio', previa=previa))
    muestras = ''.join(
        f'<span style="width: 40px; height: 40px; border-radius: 20px; background: {color}; flex: 0 0 auto;"></span>'
        for color in ACENTOS
    )
    return f'''      <div style="display: flex; height: 100%;">
{navegacion_de_ajustes('Apariencia')}
          <div style="display: flex; flex-direction: column; gap: 24px; flex: 1 1 auto; min-width: 0; padding: 32px 40px 32px 24px; overflow: hidden;">
            <div style="display: flex; flex-direction: column; gap: 6px;">
              <h2 class="serif" style="margin: 0; font-size: 32px; font-weight: 600;">Apariencia</h2>
              <span style="font-size: 14px; color: var(--on-sf-v);">Un tema cambia el color, la letra, el movimiento y el redondeo.</span>
            </div>

            <div style="display: flex; flex-direction: column; gap: 14px;">
              <span class="etiqueta-sec">Tema</span>
              <div style="display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 14px;">
{chr(10).join(tarjetas)}
              </div>
            </div>

            <div style="display: flex; flex-direction: column; gap: 14px; padding: 20px 24px; border-radius: 20px; background: var(--sf-c);">
              <div style="display: flex; flex-direction: column; gap: 4px;">
                <span style="font-size: 15px; font-weight: 600;">Color de Acento</span>
                <span style="font-size: 13px; color: var(--on-sf-v);">Sin elegir, cada tema trae el suyo. Uno propio pasa a ser la semilla y la paleta entera se deriva de él, así que el contraste sigue medido.</span>
              </div>
              <div style="display: flex; align-items: center; gap: 12px;">
                {muestras}
                <span style="display: flex; align-items: center; justify-content: center; width: 40px; height: 40px; border-radius: 20px; background: var(--sf-high); color: var(--on-sf-v);">{ico('mas', 22)}</span>
                <div style="flex: 1 1 auto;"></div>
                <span class="campo" style="width: 210px; height: 44px; background: var(--sf-high); color: var(--on-sf); font-size: 14px;"><span style="width: 18px; height: 18px; border-radius: 9px; background: #D97757; flex: 0 0 auto;"></span><span style="font-family: ui-monospace, monospace;">#D97757</span><span style="color: var(--on-sf-v); font-size: 12px;">del tema</span></span>
              </div>
            </div>
          </div>
      </div>'''


def fila_de_estado(icono, nombre, detalle, estado, etiqueta):
    return (
        f'                <div style="display: flex; align-items: center; gap: 14px; padding: 10px 0;">\n'
        f'                  <span style="display: flex; align-items: center; justify-content: center; width: 40px; height: 40px; border-radius: 12px; '
        f'background: var(--sf-high); color: var(--on-sf-v); flex: 0 0 auto;">{ico(icono, 20)}</span>\n'
        f'                  <div style="display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1 1 auto;">\n'
        f'                    <span style="font-size: 14px; font-weight: 600;">{e(nombre)}</span>\n'
        f'                    <span class="dos-lineas" style="font-size: 12px; line-height: 1.4; color: var(--on-sf-v);">{e(detalle)}</span>\n'
        f'                  </div>\n'
        f'                  <span class="estado" data-e="{estado}"><span class="punto"></span>{etiqueta}</span>\n'
        f'                </div>'
    )


def pantalla_acerca_de():
    especificaciones = ''.join(
        f'<span style="font-size: 13px; color: var(--on-sf-v);">{e(k)}</span><span style="font-size: 13px; font-weight: 500;">{e(v)}</span>'
        for k, v in [
            ('Versión', '1.0.0'),
            ('Esquema de la base', '28'),
            ('Licencia', 'Apache-2.0'),
            ('Plataforma', 'Linux · x86_64'),
            ('Motor de la interfaz', 'WebKitGTK'),
            ('Carpeta de datos', '~/.local/share/nb-books'),
            ('Lo derivado ocupa', '4 % de tu biblioteca · límite 20 %'),
        ]
    )
    estados = '\n'.join([
        fila_de_estado('almacenamiento', 'Base de datos', 'Esquema al día y sin cierres sucios.', 'listo', 'Listo'),
        fila_de_estado('lector', 'Lector', 'EPUB, PDF y cómic disponibles.', 'listo', 'Listo'),
        fila_de_estado('buscar', 'Búsqueda', 'Incompleta hasta que termine el índice: 20.687 de 151.411.', 'degradado', 'Degradado'),
        fila_de_estado('extensiones', 'Extensiones', 'Cinco instaladas, todas dentro de sus permisos.', 'listo', 'Listo'),
    ])
    return f'''      <div style="display: flex; height: 100%;">
{navegacion_de_ajustes('Acerca de')}
          <div style="display: flex; flex-direction: column; gap: 16px; flex: 1 1 auto; min-width: 0; padding: 32px 40px 32px 24px; overflow: hidden;">
            <div style="display: flex; flex-direction: column; gap: 6px; margin-bottom: 8px;">
              <h2 class="serif" style="margin: 0; font-size: 32px; font-weight: 600;">Acerca de</h2>
              <span style="font-size: 14px; color: var(--on-sf-v);">Versión, estado y diagnóstico de esta instalación.</span>
            </div>

            <div style="display: flex; align-items: center; gap: 22px; padding: 24px 28px; border-radius: 28px; background: var(--sf-c);">
              <div style="display: flex; align-items: center; justify-content: center; width: 88px; height: 88px; border-radius: 44px; background: var(--p); box-shadow: var(--elev-3); flex: 0 0 auto;">
                <div class="marca-silueta" style="width: 54px; height: 54px;"></div>
              </div>
              <div style="display: flex; flex-direction: column; gap: 6px; min-width: 0;">
                <span class="serif" style="font-size: 30px; font-weight: 600; line-height: 1.1;">NB Books</span>
                <span style="font-size: 14px; color: var(--on-sf-v);">Versión 1.0.0 · Esquema 28 · Apache-2.0</span>
              </div>
              <div style="flex: 1 1 auto;"></div>
              <span class="boton-neutro" style="height: 44px; border-radius: 22px; background: var(--sf-high);">{ico('copiar', 18)}Copiar</span>
            </div>

            <div style="display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; align-items: stretch;">
              <div style="display: flex; flex-direction: column; gap: 14px; padding: 20px 24px; border-radius: 24px; background: var(--sf-c);">
                <span style="font-size: 15px; font-weight: 600;">Especificaciones</span>
                <div style="display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 12px 20px;">{especificaciones}</div>
              </div>
              <div style="display: flex; flex-direction: column; gap: 4px; padding: 20px 24px 12px; border-radius: 24px; background: var(--sf-c);">
                <span style="font-size: 15px; font-weight: 600;">Estado del Runtime</span>
                <span style="font-size: 12px; line-height: 1.45; color: var(--on-sf-v);">Cómo está cada parte de la aplicación ahora mismo.</span>
{estados}
              </div>
            </div>

            <div style="display: flex; align-items: center; gap: 24px; padding: 20px 24px; border-radius: 24px; background: var(--sf-c);">
              <div style="display: flex; flex-direction: column; gap: 6px; min-width: 0; flex: 1 1 auto;">
                <span style="font-size: 15px; font-weight: 600;">Diagnóstico</span>
                <span style="font-size: 13px; line-height: 1.5; color: var(--on-sf-v);">Lleva la versión, el esquema, el estado de los componentes y las últimas líneas del registro. Las líneas que llevaban una ruta tuya, una credencial o un correo se retiran y se dice por qué.</span>
              </div>
              <span class="boton-tonal" style="height: 44px; border-radius: 22px;">Preparar un Diagnóstico</span>
            </div>
          </div>
      </div>'''


# ---------------------------------------------------------------------------
# Menús de la barra superior, abiertos sobre la Biblioteca
# ---------------------------------------------------------------------------

def base_de_biblioteca():
    chips = ''.join(
        f'<span class="chip" data-sel="{"true" if i == 0 else "false"}">{ico("comprobado", 16) if i == 0 else ""}{t}</span>'
        for i, t in enumerate(['Todos', 'Empezados', 'Duplicados', 'No disponibles'])
    )
    celdas = '\n'.join(celda_de_libro(t, a, {k: v for k, v in d.items() if k != 'sel'}) for t, a, d in LIBROS[:12])
    return f'''      <div style="display: flex; flex-direction: column; gap: 20px; height: 100%; padding: 20px 32px 12px; box-sizing: border-box;">
        <div style="display: flex; align-items: center; gap: 8px;">
          {chips}
          <span style="margin-left: 8px; font-size: 13px; color: var(--on-sf-v);">151.411 documentos</span>
          <div style="flex: 1 1 auto;"></div>
          <span class="selector"><span class="selector-prefijo">Ordenar por</span> Añadido {ico('abajo', 18)}</span>
          <span class="segmentado">
            <span class="segmento" data-sel="true">{ico('cuadricula', 20)}</span>
            <span class="segmento">{ico('lista', 20)}</span>
            <span class="segmento">{ico('mosaico', 20)}</span>
          </span>
        </div>
        <div style="position: relative; flex: 1 1 auto; min-height: 0; overflow: hidden;">
          <div style="display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 28px 20px; padding: 4px;">
{celdas}
          </div>
          <div style="position: absolute; left: 0; right: 0; bottom: 0; height: 80px; pointer-events: none; background: linear-gradient(180deg, transparent, var(--sf-low));"></div>
        </div>
      </div>'''


def menu_importar():
    cifras = ''.join(
        f'<div style="display: flex; flex-direction: column; gap: 2px;"><span style="font-size: 18px; font-weight: 600;">{n}</span>'
        f'<span style="font-size: 12px; color: var(--on-sf-v);">{t}</span></div>'
        for n, t in [('1.204', 'revisados'), ('1.117', 'nuevos'), ('81', 'ya estaban'), ('6', 'rechazados')]
    )
    return f'''  <div class="menu" style="right: 16px; top: 76px; width: 440px; padding: 20px; gap: 16px;">
    <div style="display: flex; align-items: center; gap: 10px;">
      {ico('importar', 22, 'color: var(--p);')}
      <span style="font-size: 16px; font-weight: 600;">Importar Documentos</span>
    </div>
    <div style="display: flex; gap: 8px;">
      <span class="boton-relleno" style="flex: 1 1 0; height: 44px; border-radius: 22px;">{ico('carpeta', 18)}Elegir Carpeta</span>
      <span class="boton-neutro" style="flex: 1 1 0; height: 44px; border-radius: 22px; background: var(--sf-highest);">{ico('importar', 18)}Elegir Documentos</span>
    </div>
    <div style="display: flex; flex-direction: column; gap: 6px;">
      <span style="font-size: 12px; font-weight: 600; color: var(--on-sf-v);">Ruta</span>
      <span class="campo" style="background: var(--sf-highest);">{ico('carpeta', 20)}<span style="color: var(--on-sf);">/mnt/libros/ensayo</span></span>
      <span style="font-size: 12px; color: var(--on-sf-v);">También puedes escribirla, o soltar archivos sobre la ventana.</span>
    </div>
    <div style="display: flex; align-items: center; gap: 16px;">
      <div style="display: flex; flex-direction: column; gap: 2px; flex: 1 1 auto;">
        <span style="font-size: 14px; font-weight: 600;">Copiar a la biblioteca</span>
        <span style="font-size: 12px; line-height: 1.4; color: var(--on-sf-v);">Sin copiar, el documento se queda donde está y solo se registra.</span>
      </div>
      <span class="interruptor"></span>
    </div>
    <div style="display: flex; flex-direction: column; gap: 12px; padding: 16px; border-radius: 16px; background: var(--sf-highest);">
      <div style="display: flex; align-items: center; gap: 8px; font-size: 13px;">
        <span style="font-weight: 600;">Importando.</span>
        <span style="color: var(--on-sf-v);">Puedes seguir usando la aplicación.</span>
      </div>
      <div class="progreso" style="height: 6px; border-radius: 3px; background: var(--sf-high);"><span style="width: 31%; border-radius: 3px;"></span></div>
      <div style="display: flex; justify-content: space-between; font-size: 12px; color: var(--on-sf-v);"><span>1.204 de 3.880</span><span class="boton-enlace" style="height: 24px; margin: -4px -8px 0 0;">Cancelar</span></div>
      <div style="display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px;">{cifras}</div>
    </div>
  </div>'''


def menu_colecciones():
    filas = ''.join(
        f'<div class="menu-item" style="{"background: var(--sf-highest);" if i == 0 else ""}">'
        f'<span style="width: 12px; height: 12px; border-radius: 4px; background: {color}; flex: 0 0 auto;"></span>'
        f'<span class="una-linea" style="flex: 1 1 auto;">{e(nombre)}</span>'
        f'<span style="font-size: 12px; color: var(--on-sf-v);">{cuantas}</span></div>'
        for i, (nombre, color, cuantas) in enumerate([
            ('Para la Tesis', 'var(--cielo)', '48'),
            ('Realismo Mágico', 'var(--melocoton)', '132'),
            ('Relecturas', 'var(--menta)', '17'),
            ('Clásicos Rusos', 'var(--rosa)', '64'),
            ('Poesía en Voz Alta', 'var(--limon)', '23'),
        ])
    )
    return f'''  <div class="menu" style="right: 167px; top: 76px; width: 320px; padding: 8px;">
    <div style="display: flex; align-items: center; height: 40px; padding: 0 16px; font-size: 12px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; color: var(--on-sf-v);">Tus Colecciones</div>
    {filas}
    <div style="height: 8px;"></div>
    <div class="menu-item" style="color: var(--p); font-weight: 600;">{ico('mas', 20)}<span>Nueva Colección</span></div>
  </div>'''


def panel_estanterias():
    def seccion(titulo, mostradas, total, filas):
        lista = ''.join(
            f'<div class="menu-item" style="min-height: 40px; padding: 0 12px;"><span class="una-linea" style="flex: 1 1 auto;">{e(n)}</span>'
            f'<span style="font-size: 12px; color: var(--on-sf-v);">{c}</span></div>'
            for n, c in filas
        )
        return (
            f'<div style="display: flex; flex-direction: column; gap: 2px;">'
            f'<div style="display: flex; align-items: baseline; gap: 8px; padding: 0 12px 4px;">'
            f'<span style="font-size: 14px; font-weight: 600;">{titulo}</span>'
            f'<span style="font-size: 12px; color: var(--on-sf-v);">{mostradas} de {total}</span></div>{lista}</div>'
        )
    return f'''  <div class="menu" style="right: 337px; top: 76px; width: 380px; padding: 16px 12px 12px; gap: 16px;">
    <span class="campo" style="margin: 0 4px; background: var(--sf-highest);">{ico('buscar', 20)}<span>Autor, serie o materia</span></span>
    {seccion('Autores', 4, '18.840', [('Jorge Luis Borges', '214'), ('Gabriel García Márquez', '96'), ('Benito Pérez Galdós', '88'), ('Isabel Allende', '61')])}
    {seccion('Series', 2, '1.203', [('Episodios Nacionales', '46'), ('La Comedia Humana', '38')])}
    {seccion('Materias', 3, '412', [('Historia', '4.560'), ('Ensayo', '3.902'), ('Realismo Mágico', '1.284')])}
    <span class="boton-enlace" style="align-self: flex-start; margin-left: 4px;">Ver toda la biblioteca</span>
  </div>'''


def desplegable_busqueda():
    def fragmento(donde, antes, termino, despues, sel=False):
        separador = '' if despues[:1] in ',.;:' else ' '
        fondo = ' background: var(--sf-highest);' if sel else ''
        return f'''    <div style="display: flex; gap: 16px; padding: 10px 16px; border-radius: 14px;{fondo}">
      <span style="width: 52px; flex: 0 0 auto; padding-top: 3px; font-size: 12px; color: var(--on-sf-v); white-space: nowrap;">{e(donde)}</span>
      <span class="serif dos-lineas" style="font-size: 15px; line-height: 1.5;">…{e(antes)} <mark style="padding: 0 2px; border-radius: 3px; background: color-mix(in srgb, var(--p) 30%, transparent); color: var(--on-sf); font-weight: 600;">{e(termino)}</mark>{separador}{e(despues)}…</span>
    </div>'''
    pestanas = ''.join(
        f'<span style="position: relative; display: flex; align-items: center; gap: 8px; height: 48px; padding: 0 16px; font-size: 14px; '
        f'font-weight: {"600" if sel else "500"}; color: {"var(--p)" if sel else "var(--on-sf-v)"};">{t}'
        f'<span style="font-size: 12px; color: var(--on-sf-v);">{n}</span>'
        + ('<span style="position: absolute; left: 12px; right: 12px; bottom: 0; height: 3px; border-radius: 3px 3px 0 0; background: var(--p);"></span>' if sel else '')
        + '</span>'
        for t, n, sel in [('Títulos', '3', False), ('Dentro de los Libros', '128', True)]
    )
    fragmentos = '\n'.join([
        fragmento('Pág. 1', 'la condición y ejercicio del famoso y valiente', 'hidalgo', 'don Quijote de la Mancha', sel=True),
        fragmento('Pág. 2', 'no ha mucho tiempo que vivía un', 'hidalgo', 'de los de lanza en astillero'),
        fragmento('Pág. 3', 'Frisaba la edad de nuestro', 'hidalgo', 'con los cincuenta años'),
        fragmento('Pág. 4', 'Es, pues, de saber que este sobredicho', 'hidalgo', ', los ratos que estaba ocioso'),
    ])
    return f'''  <div class="menu" style="left: 316px; top: 76px; width: 612px; padding: 4px 8px 12px;">
    <div style="display: flex; align-items: flex-end; gap: 4px; padding: 0 8px;">{pestanas}</div>
    <div style="display: flex; align-items: center; gap: 14px; padding: 14px 16px 8px;">
      {portada('Don Quijote de la Mancha', ancho=36, tam=1, relleno=0, con_pie=False, estilo='border-radius: 6px; box-shadow: none; color: transparent;')}
      <div style="display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1 1 auto;">
        <span class="serif una-linea" style="font-size: 15px; font-weight: 600;">Don Quijote de la Mancha</span>
        <span class="una-linea" style="font-size: 12px; color: var(--on-sf-v);">Miguel de Cervantes · 41 coincidencias</span>
      </div>
      <span class="boton-enlace">Abrir</span>
    </div>
{fragmentos}
    <div style="display: flex; align-items: center; gap: 8px; height: 44px; padding: 0 8px 0 16px; font-size: 13px; color: var(--on-sf-v);">
      <span>Y 87 coincidencias más en otras 23 obras</span>
      <div style="flex: 1 1 auto;"></div>
      <span class="boton-enlace">Ver Todas {ico('derecha', 16)}</span>
    </div>
    <div style="display: flex; align-items: center; gap: 10px; margin: 2px 8px 0; padding: 12px 16px; border-radius: 14px; background: var(--sf-highest); font-size: 12px; line-height: 1.45; color: var(--on-sf-v);">
      {ico('progreso', 18, 'color: var(--p); flex: 0 0 auto;')}
      <span>El índice va por el 14 %: dentro de los libros todavía no encuentra todo. Tarda una vez, y después es inmediata.</span>
    </div>
  </div>'''


# ---------------------------------------------------------------------------
# Registro de pantallas
# ---------------------------------------------------------------------------

PANTALLAS = {
    'Biblioteca': lambda: artboard(barra_lateral('biblioteca'), barra_superior(estanterias=True), pantalla_biblioteca()),
    'Estudio': lambda: artboard(barra_lateral('estudio'), barra_superior(), pantalla_estudio()),
    'Descubrir': lambda: artboard(barra_lateral('descubrir'), barra_superior(), pantalla_descubrir()),
    'Ajustes': lambda: artboard(barra_lateral('ajustes'), barra_superior(), pantalla_ajustes_apariencia()),
    'AcercaDe': lambda: artboard(barra_lateral('ajustes'), barra_superior(), pantalla_acerca_de()),
    'MenuImportar': lambda: artboard(barra_lateral('biblioteca'), barra_superior(estanterias=True, abierto='importar'), base_de_biblioteca(), capa=menu_importar()),
    'MenuColecciones': lambda: artboard(barra_lateral('biblioteca'), barra_superior(estanterias=True, abierto='colecciones'), base_de_biblioteca(), capa=menu_colecciones()),
    'MenuEstanterias': lambda: artboard(barra_lateral('biblioteca'), barra_superior(estanterias=True, abierto='estanterias'), base_de_biblioteca(), capa=panel_estanterias()),
    'MenuBusqueda': lambda: artboard(barra_lateral('biblioteca'), barra_superior(estanterias=True, consulta='hidalgo'), base_de_biblioteca(), capa=desplegable_busqueda()),
    'Lector': lambda: artboard(
        barra_lateral('lector', lector=True, leyendo=False),
        barra_superior_del_lector('Don Quijote de la Mancha', 'Miguel de Cervantes · Primera Parte'),
        pantalla_lector(),
    ),
}


def corregir_orden_de_inicio():
    """Estudio va antes que Descubrir, como en destinos.ts. Idempotente."""
    for nombre in ('Main.dc.html', 'Chromebook.dc.html'):
        ruta = BASE / nombre
        texto = ruta.read_text()
        bloque = re.compile(r'    <div class="destino">\n      <svg class="ico" viewBox="0 0 24 24">(?:(?!</div>\n).)*?<span>(Descubrir|Estudio)</span>.*?\n    </div>\n', re.S)
        encontrados = list(bloque.finditer(texto))
        nombres = [m.group(1) for m in encontrados]
        if nombres == ['Descubrir', 'Estudio']:
            d, s = encontrados
            texto = texto[: d.start()] + s.group(0) + d.group(0) + texto[s.end():]
            ruta.write_text(texto)
            print(f'{nombre}: Estudio antes que Descubrir')


if __name__ == '__main__':
    corregir_orden_de_inicio()
    pedidas = sys.argv[1:] or list(PANTALLAS)
    for nombre in pedidas:
        (BASE / f'{nombre}.dc.html').write_text(PANTALLAS[nombre]())
        print(f'{nombre}.dc.html escrito')
