"""Click one native GTK decoration on an exact owned fixture window."""
import ctypes as c
import json
import re
import subprocess
import sys
import time
import gi

gi.require_version('Gdk', '3.0')
gi.require_version('GdkX11', '3.0')
from gi.repository import Gdk, GdkX11

pid, xid, action = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
assert action in ('maximize', 'minimize', 'iconify', 'restore', 'close')
owner = subprocess.check_output(['xprop', '-id', str(xid), '_NET_WM_PID'], text=True, timeout=5)
assert re.search(r'=\s*' + str(pid) + r'\s*$', owner), 'Window owner differs from fixture process'
display = Gdk.Display.get_default()
window = GdkX11.X11Window.foreign_new_for_display(display, xid)
assert window
if action == 'iconify':
    window.iconify()
    display.flush()
elif action == 'restore':
    window.deiconify()
    window.show()
    window.raise_()
    display.flush()
else:
    _, left, top = window.get_origin()
    width = window.get_width()
    # Standard GTK decoration slots, measured in the native client capture.
    offset = {'maximize': 69, 'minimize': 109, 'close': 29}[action]
    x = c.CDLL('libX11.so.6')
    xt = c.CDLL('libXtst.so.6')
    x.XOpenDisplay.argtypes = [c.c_char_p]
    x.XOpenDisplay.restype = c.c_void_p
    x.XFlush.argtypes = [c.c_void_p]
    x.XCloseDisplay.argtypes = [c.c_void_p]
    xt.XTestFakeMotionEvent.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_ulong]
    xt.XTestFakeButtonEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
    connection = x.XOpenDisplay(None)
    assert connection
    try:
        assert xt.XTestFakeMotionEvent(connection, -1, left + width - offset, top + 24, 0)
        assert xt.XTestFakeButtonEvent(connection, 1, 1, 0)
        assert xt.XTestFakeButtonEvent(connection, 1, 0, 0)
        x.XFlush(connection)
    finally:
        x.XCloseDisplay(connection)
time.sleep(.3)
receipt = {'action': action, 'pid': pid, 'windowId': xid}
if action != 'close':
    receipt.update(geometry=list(window.get_geometry()), origin=list(window.get_origin()), state=subprocess.check_output(['xprop', '-id', str(xid), '_NET_WM_STATE', 'WM_STATE'], text=True, timeout=5))
print(json.dumps(receipt))
