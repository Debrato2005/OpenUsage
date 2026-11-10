"""Pause only an owned Linux x86_64 scanner at a source-read syscall boundary."""
import ctypes
import os
from pathlib import Path
import signal
import sys
import tempfile
import time

LIBC = ctypes.CDLL(None, use_errno=True) if sys.platform == 'linux' else None
if LIBC is not None:
    LIBC.ptrace.argtypes = [ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p, ctypes.c_void_p]
    LIBC.ptrace.restype = ctypes.c_long
TRACEME, PEEKDATA, GETREGS, SYSCALL, SETOPTIONS = 0, 2, 12, 24, 0x4200
TRACESYSGOOD, EXITKILL = 1, 0x100000


class Registers(ctypes.Structure):
    _fields_ = [(name, ctypes.c_ulonglong) for name in
                'r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax rip cs eflags rsp ss fs_base gs_base ds es fs gs'.split()]


def ptrace(request, pid, address=0, data=0):
    if LIBC is None:
        raise RuntimeError('The controlled scanner requires Linux')
    ctypes.set_errno(0)
    result = LIBC.ptrace(request, pid, address, data)
    if result == -1 and ctypes.get_errno():
        raise OSError(ctypes.get_errno(), 'Owned scanner tracing failed')
    return result


def source_path(pid, address):
    content = bytearray()
    for offset in range(0, 4096, 8):
        word = ptrace(PEEKDATA, pid, address + offset)
        chunk = (word & ((1 << 64) - 1)).to_bytes(8, 'little')
        if b'\0' in chunk:
            return os.fsdecode(bytes(content) + chunk.split(b'\0')[0])
        content.extend(chunk)
    raise ValueError('The scanner path exceeds the fixture bound')


def wait_owned(pid, deadline):
    while time.monotonic() < deadline:
        found, status = os.waitpid(pid, os.WNOHANG)
        if found:
            return status
        time.sleep(0.001)
    raise TimeoutError('The controlled scanner exceeded its deadline')


def scan_during_write(binary, root, selected_file, stage, writer):
    deadline = time.monotonic() + 30
    with tempfile.TemporaryDirectory(prefix='scanner-home-', dir=root) as home, \
            tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        args = [str(binary), 'pi', 'snapshot', '--json', '--offline', '--mode', 'calculate',
                '--timezone', 'UTC', '--no-color']
        env = {'HOME': home, 'PI_AGENT_DIR': str(root / 'sessions'), 'TZ': 'UTC',
               'NO_COLOR': '1', 'LOG_LEVEL': '0'}
        pid = os.fork()
        if pid == 0:
            try:
                os.setsid()
                os.chdir(home)
                os.dup2(stdout.fileno(), 1)
                os.dup2(stderr.fileno(), 2)
                with open(os.devnull, 'rb') as stdin:
                    os.dup2(stdin.fileno(), 0)
                ptrace(TRACEME, 0)
                os.kill(os.getpid(), signal.SIGSTOP)
                os.execve(binary, args, env)
            except BaseException:
                os._exit(127)
        reaped = False
        triggered = False
        source_fd = None
        opening_source = False
        entering = True
        try:
            status = wait_owned(pid, deadline)
            reaped = os.WIFEXITED(status) or os.WIFSIGNALED(status)
            if not os.WIFSTOPPED(status) or os.WSTOPSIG(status) != signal.SIGSTOP:
                raise AssertionError('The owned scanner did not enter its initial stop')
            ptrace(SETOPTIONS, pid, 0, TRACESYSGOOD | EXITKILL)
            ptrace(SYSCALL, pid)
            while True:
                status = wait_owned(pid, deadline)
                if os.WIFEXITED(status) or os.WIFSIGNALED(status):
                    reaped = True
                    code = os.waitstatus_to_exitcode(status)
                    break
                stopped = os.WSTOPSIG(status)
                if stopped == (signal.SIGTRAP | 0x80):
                    registers = Registers()
                    ptrace(GETREGS, pid, 0, ctypes.byref(registers))
                    if entering and registers.orig_rax == 437:  # Linux x86_64 openat2
                        opening_source = source_path(pid, registers.rsi) == selected_file.name
                        if opening_source and stage == 'before_open' and not triggered:
                            writer()
                            triggered = True
                    elif not entering and registers.orig_rax == 437 and opening_source:
                        if registers.rax < (1 << 63):
                            source_fd = registers.rax
                        opening_source = False
                    elif (not entering and registers.orig_rax == 0 and registers.rdi == source_fd
                          and 0 < registers.rax < (1 << 63) and stage == 'after_read' and not triggered):
                        writer()
                        triggered = True
                    entering = not entering
                    ptrace(SYSCALL, pid)
                else:
                    ptrace(SYSCALL, pid, 0, 0 if stopped == signal.SIGTRAP else stopped)
            if not triggered:
                raise AssertionError('The selected source-read boundary did not occur')
            stdout.seek(0)
            stderr.seek(0)
            output, errors = stdout.read(128 * 1024 + 1), stderr.read(128 * 1024 + 1)
            if len(output) + len(errors) > 128 * 1024:
                raise AssertionError('The scanner exceeded the fixture output bound')
            if list(Path(home).iterdir()):
                raise AssertionError('The scanner wrote into its private home')
            return code, output, errors
        finally:
            if not reaped:
                os.killpg(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
