#!/bin/sh
# Panel 0.2.4 (QD-129): builds data/panel.gresource from panel.gresource.xml
# (checked in; the build needs no glib-compile-resources). Run after a
# change of the logo SVG; the unit test in logo.rs compares the contents.
set -eu
cd "$(dirname "$0")"
glib-compile-resources --target=panel.gresource panel.gresource.xml
sha256sum panel.gresource
