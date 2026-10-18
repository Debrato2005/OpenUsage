"""Send WM_DELETE_WINDOW to one owned application's X11 window, without signals."""
import ctypes as c
import json
import os
import sys

pid = int(sys.argv[1])
assert pid > 1
x = c.CDLL('libX11.so.6')
Display = c.c_void_p
Window = c.c_ulong
Atom = c.c_ulong
x.XOpenDisplay.argtypes = [c.c_char_p]
x.XOpenDisplay.restype = Display
x.XDefaultRootWindow.argtypes = [Display]
x.XDefaultRootWindow.restype = Window
x.XInternAtom.argtypes = [Display, c.c_char_p, c.c_int]
x.XInternAtom.restype = Atom
x.XQueryTree.argtypes = [Display, Window, c.POINTER(Window), c.POINTER(Window), c.POINTER(c.POINTER(Window)), c.POINTER(c.c_uint)]
x.XGetWindowProperty.argtypes = [Display, Window, Atom, c.c_long, c.c_long, c.c_int, Atom, c.POINTER(Atom), c.POINTER(c.c_int), c.POINTER(c.c_ulong), c.POINTER(c.c_ulong), c.POINTER(c.POINTER(c.c_ubyte))]
x.XGetWMProtocols.argtypes = [Display, Window, c.POINTER(c.POINTER(Atom)), c.POINTER(c.c_int)]
x.XFree.argtypes = [c.c_void_p]
x.XFlush.argtypes = [Display]
x.XCloseDisplay.argtypes = [Display]

class Data(c.Union):
    _fields_ = [('b', c.c_char * 20), ('s', c.c_short * 10), ('l', c.c_long * 5)]

class ClientMessage(c.Structure):
    _fields_ = [('type', c.c_int), ('serial', c.c_ulong), ('send_event', c.c_int), ('display', Display), ('window', Window), ('message_type', Atom), ('format', c.c_int), ('data', Data)]

class Event(c.Union):
    _fields_ = [('xclient', ClientMessage), ('padding', c.c_long * 24)]

x.XSendEvent.argtypes = [Display, Window, c.c_int, c.c_long, c.POINTER(Event)]
display = x.XOpenDisplay(None)
assert display, 'No isolated X11 display.'
try:
    pid_atom = x.XInternAtom(display, b'_NET_WM_PID', 1)
    protocols = x.XInternAtom(display, b'WM_PROTOCOLS', 1)
    delete = x.XInternAtom(display, b'WM_DELETE_WINDOW', 1)
    assert pid_atom and protocols and delete
    pending = [x.XDefaultRootWindow(display)]
    matches = []
    while pending:
        window = pending.pop()
        root, parent, children, count = Window(), Window(), c.POINTER(Window)(), c.c_uint()
        if x.XQueryTree(display, window, c.byref(root), c.byref(parent), c.byref(children), c.byref(count)):
            pending.extend(children[i] for i in range(count.value))
            if children:
                x.XFree(children)
        actual, fmt, items, remaining, data = Atom(), c.c_int(), c.c_ulong(), c.c_ulong(), c.POINTER(c.c_ubyte)()
        status = x.XGetWindowProperty(display, window, pid_atom, 0, 1, 0, 0, c.byref(actual), c.byref(fmt), c.byref(items), c.byref(remaining), c.byref(data))
        owned = status == 0 and fmt.value == 32 and items.value == 1 and c.cast(data, c.POINTER(c.c_ulong))[0] == pid
        if data:
            x.XFree(data)
        if not owned:
            continue
        values, length = c.POINTER(Atom)(), c.c_int()
        if x.XGetWMProtocols(display, window, c.byref(values), c.byref(length)):
            if delete in [values[i] for i in range(length.value)]:
                matches.append(window)
            if values:
                x.XFree(values)
    assert len(matches) == 1, f'Expected one owned closeable X11 window, found {len(matches)}.'
    event = Event()
    event.xclient.type = 33
    event.xclient.send_event = 1
    event.xclient.display = display
    event.xclient.window = matches[0]
    event.xclient.message_type = protocols
    event.xclient.format = 32
    event.xclient.data.l[0] = delete
    event.xclient.data.l[1] = 0
    assert x.XSendEvent(display, matches[0], 0, 0, c.byref(event)), 'X11 refused WM_DELETE_WINDOW.'
    x.XFlush(display)
    print(json.dumps({'applicationPid': pid, 'windowId': matches[0], 'method': 'X11 WM_PROTOCOLS WM_DELETE_WINDOW', 'display': os.environ.get('DISPLAY')}))
finally:
    x.XCloseDisplay(display)
