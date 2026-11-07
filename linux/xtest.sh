#!/bin/bash
# Visual smoke test inside the container: Xvfb + openbox, run the app, hover the pill, take screenshots.
set -e
export DISPLAY=:99 XDG_CONFIG_HOME=/tmp/cfg XDG_DATA_HOME=/tmp/data
mkdir -p $XDG_CONFIG_HOME/notchhub shots
printf "halo_always = ${HALO:-true}\n" > $XDG_CONFIG_HOME/notchhub/settings.toml
[ -n "$SFILE" ] && cat "$SFILE" >> $XDG_CONFIG_HOME/notchhub/settings.toml
[ -n "$LASTTAB" ] && echo "last_tab = \"$LASTTAB\"" >> $XDG_CONFIG_HOME/notchhub/settings.toml
mkdir -p /tmp/files $XDG_DATA_HOME/notchhub
convert -size 200x200 gradient:'#ff7a59-#3da9ff' /tmp/files/photo.png
convert -size 200x200 xc:'#32d74b' /tmp/files/green.jpg
echo hello > /tmp/files/notes.txt
T=$(date +%Y%m%d); N=$(date -d tomorrow +%Y%m%d)
printf "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:Reunion de equipo\nLOCATION:Sala 2\nDTSTART:${T}T170000\nDTEND:${T}T180000\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:Llamar al dentista\nDTSTART:${T}T190000\nDTEND:${T}T193000\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:Cena\nDTSTART:${N}T210000\nEND:VEVENT\nEND:VCALENDAR\n" > /tmp/files/test.ics
echo '{"items":["/tmp/files/photo.png","/tmp/files/notes.txt","/tmp/files/green.jpg"]}' > $XDG_DATA_HOME/notchhub/shelf.json
echo '{"entries":[{"text":"cargo build --release","pinned":true,"time":1},{"text":"https://example.com/some/long/link?with=params","pinned":false,"time":2},{"text":"Una frase copiada de prueba","pinned":false,"time":3}]}' > $XDG_DATA_HOME/notchhub/clipboard.json
Xvfb :99 -screen 0 1280x720x24 >/dev/null 2>&1 &
sleep 1
openbox >/dev/null 2>&1 &
sleep 1
GDK_BACKEND=x11 python3 tests/bg.py >/dev/null 2>&1 &
sleep 1.5
${COMPOSITOR:-picom} >/dev/null 2>&1 &
sleep 1
dbus-run-session -- bash -c "[ -n \"$MOCK\" ] && python3 tests/mock_player.py >/tmp/mock.log 2>&1 & exec target/debug/notchhub" >/tmp/app.log 2>&1 &
sleep 3
import -window root shots/1-collapsed.png
xdotool mousemove 100 400; sleep 0.5; xdotool mousemove 640 2; sleep 0.6
import -window root shots/2a-open.png
sleep 1
import -window root shots/2-open.png
xdotool mousemove 700 120; sleep 1.5
import -window root shots/2b-open.png
xwininfo -root -tree | head -30
xdotool mousemove 100 400; sleep 1
import -window root shots/3-closed.png
xwininfo -root -tree | grep -i notch || true
cat /tmp/app.log
