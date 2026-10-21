#!/usr/bin/env python3
"""Private synthetic Desktop Notifications service. Never connects to a user bus."""
import json
import os
from pathlib import Path
import signal
import sys

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

os.umask(0o077)
root = Path(sys.argv[1])
repository = Path(__file__).resolve().parents[2]
assert root.resolve() == root and root.parent == repository / '.recovery/tmp'
assert root.name.startswith('n-') and not root.is_symlink()
assert root.stat().st_uid == os.getuid() and root.stat().st_mode & 0o777 == 0o700
assert os.environ['DBUS_SESSION_BUS_ADDRESS'].startswith(f'unix:path={root}/runtime/')
control = root / 'control'
assert control.is_dir() and not control.is_symlink()
log = control / 'notification-submissions.jsonl'
loop = GLib.MainLoop()
DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus(private=True)
name = dbus.service.BusName('org.freedesktop.Notifications', bus=bus, do_not_queue=True)

class Notifications(dbus.service.Object):
    def __init__(self):
        super().__init__(name, '/org/freedesktop/Notifications')
        self.count = 0

    @dbus.service.method('org.freedesktop.Notifications', in_signature='', out_signature='as')
    def GetCapabilities(self):
        return ['body', 'body-markup']

    @dbus.service.method('org.freedesktop.Notifications', in_signature='', out_signature='ssss')
    def GetServerInformation(self):
        return ('OpenUsage private test service', 'OpenUsage test harness', '1', '1.2')

    @dbus.service.method('org.freedesktop.Notifications', in_signature='susssasa{sv}i', out_signature='u')
    def Notify(self, application, replaces, icon, title, body, actions, hints, timeout):
        assert application == 'OpenUsage' and replaces == 0
        assert icon == 'app.openusage.desktop' and not actions
        assert hints.get('desktop-entry') == 'app.openusage.desktop'
        assert 'OpenUsage' in body and len(title) < 256 and len(body) < 2048
        self.count += 1
        record = {'id': self.count, 'application': str(application), 'title': str(title), 'body': str(body)}
        fd = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
        try:
            metadata = os.fstat(fd)
            assert metadata.st_uid == os.getuid() and metadata.st_mode & 0o777 == 0o600 and metadata.st_nlink == 1
            os.write(fd, (json.dumps(record) + '\n').encode())
            os.fsync(fd)
        finally:
            os.close(fd)
        return dbus.UInt32(self.count)

notifications = Notifications()
signal.signal(signal.SIGTERM, lambda *_: loop.quit())
print('private-notifications-ready', flush=True)
try:
    loop.run()
finally:
    bus.close()
