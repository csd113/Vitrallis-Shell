#!/usr/bin/env python3
"""Import explicitly selected saved data into an absent Vitrallis AppData directory.

Apps must be closed. Preview is the default; sources are always preserved.
This is an administrator-assisted, one-time import, not a runtime fallback.
"""
import argparse
import ctypes
import errno
import os
from pathlib import Path
import pwd
import re
import shutil
import stat
import sys
import tempfile


def absolute(path):
    path = Path(path)
    if (not path.is_absolute() or '..' in path.parts
            or any(ord(c) < 32 or ord(c) == 127 for c in str(path))):
        raise ValueError('Use an absolute path without traversal or control characters')
    return path


def directories(path):
    for parent in (path, *path.parents):
        try:
            info = parent.lstat()
        except FileNotFoundError:
            continue
        if (not stat.S_ISDIR(info.st_mode)
                or info.st_mode & 0o022 and not info.st_mode & stat.S_ISVTX):
            raise ValueError('Unsafe directory: ' + str(parent))


def relative(path, tree=False):
    path = Path(path)
    if (path.is_absolute() or '..' in path.parts or not path.parts and not tree
            or any(ord(c) < 32 or ord(c) == 127 for c in str(path))):
        raise ValueError('Invalid relative destination')
    return path


def snapshot(path):
    info = path.lstat()
    if (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1
            or info.st_uid != os.getuid() or info.st_mode & 0o022):
        raise ValueError('Expected an owned regular saved-data file: ' + str(path))
    return info


def identity(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def private_directories(path):
    directories(path)
    missing = []
    while not path.exists():
        missing.append(path)
        path = path.parent
    for directory in reversed(missing):
        directory.mkdir(mode=0o700)


def plan(home, app_id, files, trees):
    home = absolute(home)
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,255}', app_id):
        raise ValueError('Invalid application ID')
    directories(home)
    if home.lstat().st_uid != os.getuid():
        raise ValueError('Home must belong to the importing user')
    destination = home / 'Documents/Vitrallis/AppData' / app_id
    directories(destination.parent)
    data_parent = destination.parent
    if data_parent.exists():
        info = data_parent.lstat()
        if info.st_uid != os.getuid() or info.st_mode & 0o077:
            raise ValueError('AppData must be private and user-owned')
    if destination.exists() or destination.is_symlink():
        raise ValueError('Destination already exists; review its data before importing')
    result, names, total, count = [], set(), 0, 0

    def add(source, target):
        nonlocal total
        info = snapshot(source)
        if target in names or any(p in names for p in target.parents):
            raise ValueError('Overlapping destination files')
        if any(target in old.parents for old in names):
            raise ValueError('Overlapping destination files')
        names.add(target)
        total += info.st_size
        if len(result) >= 100_000 or total > 2 * 1024**3:
            raise ValueError('Import exceeds 100,000 files or 2 GiB')
        result.append((source, target, info))

    for source, target in files:
        source = absolute(source)
        directories(source.parent)
        add(source, relative(target))
    for source, target in trees:
        source, target = absolute(source), relative(target, tree=True)
        directories(source)
        if source.lstat().st_uid != os.getuid():
            raise ValueError('Source directory must belong to the importing user')
        pending = [(source, target, 0)]
        while pending:
            directory, output, depth = pending.pop()
            if depth >= 64:
                raise ValueError('Import exceeds 64 directory levels')
            with os.scandir(directory) as stream:
                for item in stream:
                    entry = Path(item.path)
                    count += 1
                    if count > 100_000:
                        raise ValueError('Import exceeds 100,000 directory entries')
                    relative(entry.name)
                    info = entry.lstat()
                    child = output / entry.name
                    if stat.S_ISDIR(info.st_mode):
                        directories(entry)
                        if info.st_uid != os.getuid():
                            raise ValueError('Source directory belongs to another user')
                        pending.append((entry, child, depth + 1))
                    else:
                        add(entry, child)
    if not result:
        raise ValueError('Select at least one saved-data file')
    if any(source == destination or destination in source.parents for source, _, _ in result):
        raise ValueError('Import sources must be outside the destination')
    return destination, result, total


def copy_file(source, output, before):
    descriptor = os.open(source, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, 'rb') as reader:
        opened = os.fstat(reader.fileno())
        if identity(opened) != identity(before) or opened.st_nlink != 1:
            raise ValueError('Saved-data file changed before copying')
        descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, 'wb') as writer:
            remaining = before.st_size
            while remaining:
                block = reader.read(min(remaining, 64 * 1024))
                if not block:
                    raise ValueError('Saved-data file became shorter while copying')
                writer.write(block)
                remaining -= len(block)
            if reader.read(1):
                raise ValueError('Saved-data file grew while copying')
            writer.flush()
            os.fsync(writer.fileno())
        if identity(source.lstat()) != identity(before):
            raise ValueError('Saved-data file changed while copying')


def publish(stage, destination):
    library = ctypes.CDLL(None, use_errno=True)
    if sys.platform == 'linux':
        rename = library.renameat2
        rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
        result = rename(-100, os.fsencode(stage), -100, os.fsencode(destination), 1)
    elif sys.platform == 'darwin':
        rename = library.renamex_np
        rename.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
        result = rename(os.fsencode(stage), os.fsencode(destination), 4)
    else:
        raise ValueError('Exclusive import requires Linux or macOS')
    if result != 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))


def apply(destination, entries, total):
    ancestor = destination.parent
    while not ancestor.exists():
        ancestor = ancestor.parent
    if shutil.disk_usage(ancestor).free < total + 32 * 1024**2:
        raise OSError(errno.ENOSPC, 'Not enough space for an import and 32 MiB reserve')
    directories(destination.parent)
    private_directories(destination.parent)
    stage = Path(tempfile.mkdtemp(prefix='.import-', dir=destination.parent))
    committed = False
    try:
        for source, target, before in entries:
            output = stage / target
            private_directories(output.parent)
            copy_file(source, output, before)
        for directory, _, _ in os.walk(stage, topdown=False):
            descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        publish(stage, destination)
        committed = True
        try:
            descriptor = os.open(destination.parent, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        except OSError as error:
            raise OSError('Imported data is present, but storage sync failed; reboot persistence is uncertain') from error
    finally:
        if not committed:
            shutil.rmtree(stage)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app-id', required=True)
    parser.add_argument('--file', nargs=2, action='append', default=[], metavar=('SOURCE', 'RELATIVE_DESTINATION'))
    parser.add_argument('--tree', nargs=2, action='append', default=[], metavar=('SOURCE', 'RELATIVE_DESTINATION'))
    parser.add_argument('--apply', action='store_true', help='Publish the validated import; preserve sources')
    args = parser.parse_args()
    try:
        if os.geteuid() == 0:
            raise ValueError('Run as the normal desktop user, without sudo')
        destination, entries, total = plan(pwd.getpwuid(os.getuid()).pw_dir, args.app_id, args.file, args.tree)
        print(f'{len(entries)} files, {total} bytes -> {destination}')
        for _, target, _ in entries:
            print('  ' + str(target))
        if args.apply:
            apply(destination, entries, total)
            print('Imported. Original files remain unchanged.')
        else:
            print('Preview only. Close the app and rerun with --apply after reviewing these paths.')
    except (OSError, ValueError, AttributeError) as error:
        parser.exit(1, str(error) + '\n')


if __name__ == '__main__':
    main()
