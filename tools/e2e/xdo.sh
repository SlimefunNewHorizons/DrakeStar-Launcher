#!/bin/bash
# Uso: xdo.sh "<acciones separadas por ;>" [nombre-de-captura]
# Acciones: click X Y | type TEXTO | key TECLA | wait SEG
export DISPLAY=:99
SHOTN="$2"
IFS=';' read -ra ACTS <<< "$1"
for a in "${ACTS[@]}"; do
  set -- $a
  case "$1" in
    click) xdotool mousemove "$2" "$3" click 1 ;;
    type) shift; xdotool type --delay 40 "$*" ;;
    key) xdotool key "$2" ;;
    wait) sleep "$2" ;;
  esac
  sleep 0.4
done
[ -n "$SHOTN" ] && import -window root "${DS_SHOTS:-/tmp}/$SHOTN.png" && echo "captura ${DS_SHOTS:-/tmp}/$SHOTN.png"
