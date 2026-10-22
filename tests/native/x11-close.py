"""Focus or close one owned application X11 window, without process signals."""
import ctypes as c
import json
import os
import sys

pid = int(sys.argv[1])
assert pid > 1
operation = sys.argv[2] if len(sys.argv) == 3 else 'close'
assert operation in {'close', 'focus'}
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
class Attributes(c.Structure):
    _fields_ = [(name, kind) for name, kind in [
        ('x', c.c_int), ('y', c.c_int), ('width', c.c_int), ('height', c.c_int), ('border_width', c.c_int), ('depth', c.c_int),
        ('visual', c.c_void_p), ('root', Window), ('window_class', c.c_int), ('bit_gravity', c.c_int), ('win_gravity', c.c_int), ('backing_store', c.c_int),
        ('backing_planes', c.c_ulong), ('backing_pixel', c.c_ulong), ('save_under', c.c_int), ('colormap', c.c_ulong), ('map_installed', c.c_int), ('map_state', c.c_int),
        ('all_event_masks', c.c_long), ('your_event_mask', c.c_long), ('do_not_propagate_mask', c.c_long), ('override_redirect', c.c_int), ('screen', c.c_void_p)]]
x.XGetWindowAttributes.argtypes = [Display, Window, c.POINTER(Attributes)]
x.XFetchName.argtypes = [Display, Window, c.POINTER(c.c_void_p)]
x.XFree.argtypes = [c.c_void_p]
x.XFlush.argtypes = [Display]
x.XCloseDisplay.argtypes = [Display]
x.XRaiseWindow.argtypes = [Display, Window]
x.XSetInputFocus.argtypes = [Display, Window, c.c_int, c.c_ulong]
x.XGetInputFocus.argtypes = [Display, c.POINTER(Window), c.POINTER(c.c_int)]
x.XSync.argtypes = [Display, c.c_int]

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
    owned_windows = []
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
        attributes, name_pointer = Attributes(), c.c_void_p()
        assert x.XGetWindowAttributes(display, window, c.byref(attributes))
        title = ''
        if x.XFetchName(display, window, c.byref(name_pointer)) and name_pointer:
            title = c.string_at(name_pointer).decode('utf8', errors='replace')
            x.XFree(name_pointer)
        if not title:
            name_atom = x.XInternAtom(display, b'_NET_WM_NAME', 1)
            name_data = c.POINTER(c.c_ubyte)()
            status = x.XGetWindowProperty(display, window, name_atom, 0, 256, 0, 0, c.byref(actual), c.byref(fmt), c.byref(items), c.byref(remaining), c.byref(name_data))
            if status == 0 and fmt.value == 8 and items.value <= 1024 and name_data:
                title = c.string_at(name_data, items.value).decode('utf8', errors='replace')
            if name_data:
                x.XFree(name_data)
        owned_windows.append({'windowId': window, 'title': title, 'mapState': attributes.map_state})
        if attributes.map_state != 2 or title not in {'OpenUsage', 'OpenUsage — Test workspace'}:
            continue
        values, length = c.POINTER(Atom)(), c.c_int()
        if x.XGetWMProtocols(display, window, c.byref(values), c.byref(length)):
            if delete in [values[i] for i in range(length.value)]:
                matches.append(window)
            if values:
                x.XFree(values)
    assert len(matches) == 1, f'Expected one visible owned OpenUsage window, found {len(matches)}; owned windows: {owned_windows}.'
    receipt = {'applicationPid': pid, 'windowId': matches[0],
               'display': os.environ.get('DISPLAY'), 'ownedWindows': owned_windows}
    if operation == 'focus':
        x.XRaiseWindow(display, matches[0])
        x.XSetInputFocus(display, matches[0], 2, 0)
        x.XSync(display, 0)
        focused, revert = Window(), c.c_int()
        x.XGetInputFocus(display, c.byref(focused), c.byref(revert))
        assert focused.value == matches[0], 'X11 did not focus the exact owned window.'
        receipt.update(method='X11 XRaiseWindow XSetInputFocus', inputFocusWindow=focused.value)
    else:
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
        receipt['method'] = 'X11 WM_PROTOCOLS WM_DELETE_WINDOW'
    print(json.dumps(receipt))
finally:
    x.XCloseDisplay(display)
