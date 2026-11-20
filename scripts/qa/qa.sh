#!/usr/bin/env bash
# Auditoría de la aplicación instalada: KWin virtual, una copia de los datos del
# usuario y el inspector remoto de WebKit. Nunca toca los datos reales: todo
# XDG_* apunta a la copia, y las raíces de la biblioteca se leen en solo lectura
# por el propio runtime.
#
# Uso: scripts/qa/qa.sh iniciar | detener
#   NB_QA_BINARIO   binario a auditar (por defecto, el instalado)
#   NB_QA_ANCHO/ALTO tamaño de la pantalla virtual
set -euo pipefail

QA=${NB_QA_ROOT:-"$HOME/.cache/nb-books-qa"}
BINARIO=${NB_QA_BINARIO:-"$HOME/.local/share/nb-books/usr/bin/nb-books"}
RUNTIME="$QA/runtime"
SOCKET=wayland-nbbooks-qa
PUERTO=${NB_QA_INSPECTOR_PORT:-9334}

iniciar() {
  mkdir -p "$RUNTIME" "$QA/registros"
  if [[ ${NB_QA_REAL:-0} == 1 ]]; then
    # Exactamente como la abre el lanzador del escritorio: por AppRun, que fuerza
    # GDK_BACKEND=x11 y su propio tema GTK. Solo cambian los datos y el inspector.
    env WEBKIT_INSPECTOR_HTTP_SERVER="127.0.0.1:$PUERTO" \
      XDG_DATA_HOME="$QA/xdg-data" XDG_CONFIG_HOME="$QA/xdg-config" \
      XDG_CACHE_HOME="$QA/xdg-cache" \
      "$(dirname "$(dirname "$(dirname "$BINARIO")")")/AppRun" >"$QA/registros/app.log" 2>&1 &
    echo $! >"$QA/app.pid"
    esperar_inspector
    return
  fi
  chmod 700 "$RUNTIME"
  # Wayland directo: el Xwayland del KWin anidado se arranca a demanda y en esta
  # máquina no llegaba a responder, así que la ventana no se creaba nunca.
  env XDG_RUNTIME_DIR="$RUNTIME" kwin_wayland --socket "$SOCKET" --virtual \
    --width "${NB_QA_ANCHO:-1440}" --height "${NB_QA_ALTO:-900}" \
    --no-lockscreen --no-global-shortcuts >"$QA/registros/kwin.log" 2>&1 &
  echo $! >"$QA/kwin.pid"
  for _ in $(seq 1 150); do
    [[ -S "$RUNTIME/$SOCKET" ]] && break
    sleep 0.1
  done
  [[ -S "$RUNTIME/$SOCKET" ]] || { echo "KWin virtual no arrancó" >&2; exit 1; }
  lanzar "$RUNTIME/$SOCKET"
}

# NB_QA_REAL=1: la ventana se abre en el escritorio de verdad, con su escala, su
# compositor y su pantalla. Lo que se mide en el KWin virtual no siempre vale aquí.
lanzar() {
  env -u DISPLAY GDK_BACKEND=wayland WAYLAND_DISPLAY="$1" \
    WEBKIT_INSPECTOR_HTTP_SERVER="127.0.0.1:$PUERTO" \
    XDG_DATA_HOME="$QA/xdg-data" XDG_CONFIG_HOME="$QA/xdg-config" \
    XDG_CACHE_HOME="$QA/xdg-cache" RUST_LOG="${NB_QA_RUST_LOG:-info}" \
    "$BINARIO" >"$QA/registros/app.log" 2>&1 &
  echo $! >"$QA/app.pid"
  esperar_inspector
}

esperar_inspector() {
  for _ in $(seq 1 300); do
    if curl -fsS "http://127.0.0.1:$PUERTO/" 2>/dev/null | grep -q socket/; then
      echo listo
      return
    fi
    sleep 0.1
  done
  echo "el inspector no apareció" >&2
  exit 1
}

detener() {
  local p
  for p in app kwin; do
    if [[ -f "$QA/$p.pid" ]]; then
      kill "$(cat "$QA/$p.pid")" 2>/dev/null || true
      rm -f "$QA/$p.pid"
    fi
  done
}

case ${1:-} in
  iniciar) iniciar ;;
  detener) detener ;;
  *) echo "uso: $0 iniciar|detener" >&2; exit 2 ;;
esac
