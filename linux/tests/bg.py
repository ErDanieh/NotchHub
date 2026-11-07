#!/usr/bin/env python3
"""A plain wallpaper behind everything, only for screenshots."""
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
import cairo

def draw(area, cr, w, h):
    g = cairo.LinearGradient(0, 0, w, h)
    g.add_color_stop_rgb(0, 0.16, 0.22, 0.34)
    g.add_color_stop_rgb(1, 0.48, 0.35, 0.55)
    cr.set_source(g)
    cr.paint()

def activate(app):
    win = Gtk.ApplicationWindow(application=app, decorated=False)
    area = Gtk.DrawingArea()
    area.set_draw_func(draw)
    win.set_child(area)
    win.set_default_size(1280, 720)
    win.fullscreen()
    win.present()

app = Gtk.Application(application_id="test.bg")
app.connect("activate", activate)
app.run()
