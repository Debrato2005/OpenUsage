"""Send Shift+Tab only to the already focused window on a private test display."""
import ctypes as c
import json
import os
import re
import sys

assert os.environ.get('OPENUSAGE_NATIVE_PRIVATE_XVFB') == '1'
assert re.fullmatch(r':\d+(?:\.\d+)?', os.environ.get('DISPLAY', ''))
expected_window = int(sys.argv[1])
assert expected_window > 0
x = c.CDLL('libX11.so.6')
xt = c.CDLL('libXtst.so.6')
Display = c.c_void_p
Window = c.c_ulong
x.XOpenDisplay.argtypes = [c.c_char_p]
x.XOpenDisplay.restype = Display
x.XGetInputFocus.argtypes = [Display, c.POINTER(Window), c.POINTER(c.c_int)]
x.XKeysymToKeycode.argtypes = [Display, c.c_ulong]
x.XKeysymToKeycode.restype = c.c_ubyte
x.XSync.argtypes = [Display, c.c_int]
x.XCloseDisplay.argtypes = [Display]
xt.XTestQueryExtension.argtypes = [Display, c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_int)]
xt.XTestFakeKeyEvent.argtypes = [Display, c.c_uint, c.c_int, c.c_ulong]
display = x.XOpenDisplay(None)
assert display, 'No private test display.'
shift = tab = 0
try:
    focused, revert = Window(), c.c_int()
    x.XGetInputFocus(display, c.byref(focused), c.byref(revert))
    assert focused.value == expected_window, 'The owned application window lost input focus.'
    event_base, error_base, major, minor = (c.c_int() for _ in range(4))
    assert xt.XTestQueryExtension(display, c.byref(event_base), c.byref(error_base), c.byref(major), c.byref(minor))
    shift = x.XKeysymToKeycode(display, 0xffe1)
    tab = x.XKeysymToKeycode(display, 0xff09)
    assert shift and tab
    for keycode, pressed in [(shift, True), (tab, True), (tab, False), (shift, False)]:
        assert xt.XTestFakeKeyEvent(display, keycode, pressed, 0)
    x.XSync(display, False)
    print(json.dumps({'method': 'XTEST Shift_L+Tab press/release', 'display': os.environ['DISPLAY'],
                      'focusedWindow': focused.value, 'shiftKeycode': shift, 'tabKeycode': tab,
                      'xtestVersion': [major.value, minor.value]}))
finally:
    if tab:
        xt.XTestFakeKeyEvent(display, tab, False, 0)
    if shift:
        xt.XTestFakeKeyEvent(display, shift, False, 0)
    x.XSync(display, False)
    x.XCloseDisplay(display)
