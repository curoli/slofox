#!/usr/bin/env bash
set -euo pipefail

if ! command -v pw-loopback >/dev/null 2>&1; then
    echo 'pw-loopback is missing. Install pipewire-bin.' >&2
    exit 1
fi

echo 'Keep this terminal open. In pavucontrol, route Firefox/Chrome playback to "Slofox Browser".'
echo 'Browser audio remains audible on the default output. Press Ctrl+C to remove this temporary sink.'

exec pw-loopback \
    --capture-props='{ node.name = "slofox_browser" node.description = "Slofox Browser" media.class = "Audio/Sink" audio.position = [ FL FR ] }' \
    --playback-props='{ node.name = "slofox_browser_monitor" node.description = "Slofox Browser Listening" node.passive = true audio.position = [ FL FR ] }'
