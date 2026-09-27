#!/bin/sh
# Renders the menu bar icons from their SVG sources: 36 px, so they're sharp
# at the 18 points macOS shows them. Needs rsvg-convert (brew install librsvg).
cd "$(dirname "$0")"
for svg in *.svg; do
  rsvg-convert -w 36 -h 36 "$svg" -o "${svg%.svg}.png"
done
