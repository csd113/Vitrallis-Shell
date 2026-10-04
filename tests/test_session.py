import importlib.util
import fcntl
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location('session', Path(__file__).resolve().parents[1] / 'integrations/pocketchip/vitrallis-session.py')
s = importlib.util.module_from_spec(spec)
spec.loader.exec_module(s)
LUA = shutil.which('lua') or shutil.which('lua5.3') or shutil.which('luajit')


class Session(unittest.TestCase):
    @unittest.skipUnless(LUA, 'requires an available Lua interpreter for Awesome hook execution')
    def test_window_readiness_claims_the_launcher_once_and_restores_only_owned_hooks(self):
        fixture = r'''
local vitrallis_home_route
local previous = {valid = true, raised = 0}
function previous:raise() self.raised = self.raised + 1 end
local original_key = {key = "XF86PowerOff", modifiers = {}}
local other_key = {key = "F1", modifiers = {}}
local keys = {original_key, other_key}
local windows = {previous}
local callbacks = {}
local filters = {}
root = {keys = function(value) if value then keys = value end; return keys end}
client = {focus = previous, get = function() return windows end,
    connect_signal = function(name, callback) callbacks[name] = callback end,
    disconnect_signal = function(name, callback)
        assert(callbacks[name] == callback); callbacks[name] = nil
    end}
require = function(name)
    assert(name == "awful")
    return {ewmh = {
        add_activate_filter = function(callback, context) filters[context] = callback end,
        remove_activate_filter = function(callback, context)
            assert(filters[context] == callback); filters[context] = nil
        end}, key = function(modifiers, name, callback)
        return {{key = name, modifiers = modifiers, callback = callback}}
    end}
end
'''
        script = fixture + '\n' + s.HOME_HOOK + r'''
assert(client.focus == previous)
local function window(name, class)
    local c = {valid = true, name = name, class = class, raised = 0}
    function c:raise() self.raised = self.raised + 1 end
    return c
end
local unrelated = window("Vitrallis", "unrelated")
callbacks.manage(unrelated)
assert(client.focus == previous)
local shell = window(nil, "vitrallis")
callbacks.manage(shell)
assert(client.focus == previous)
shell.name = "Vitrallis"
callbacks["property::name"](shell)
assert(client.focus == shell and shell.raised == 1)
local app = window("Notepad", "vitrallis-notepad")
client.focus = app
callbacks["property::name"](shell)
assert(client.focus == app and shell.raised == 1)
shell.valid = false
local replacement = window("Vitrallis", "vitrallis")
callbacks.manage(replacement)
assert(client.focus == replacement and replacement.raised == 1)
local concurrent_key = {key = "F12", modifiers = {}}
table.insert(keys, concurrent_key)
''' + '\n' + s.RESTORE_HOOK + r'''
assert(callbacks.manage == nil and callbacks["property::name"] == nil)
assert(callbacks.focus == nil)
assert(filters.rules == nil and filters.ewmh == nil)
assert(client.focus == previous and previous.raised == 1)
assert(vitrallis_home_route == nil)
assert(#keys == 3 and keys[1] == other_key and keys[2] == concurrent_key and keys[3] == original_key)
'''
        # Each hook returns a status string; give it a function scope so both
        # execute in one sandbox with the same mocked Awesome client lifecycle.
        script = script.replace(s.HOME_HOOK, 'do local status = (function()\n' + s.HOME_HOOK + '\nend)() end')
        script = script.replace(s.RESTORE_HOOK, 'do local status = (function()\n' + s.RESTORE_HOOK + '\nend)() end')
        result = subprocess.run([LUA, '-'], input=script, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)

    @unittest.skipUnless(LUA, 'requires an available Lua interpreter for Awesome hook execution')
    def test_delayed_desktop_windows_cannot_steal_focus_and_filters_are_reversible(self):
        fixture = r'''
local vitrallis_home_route
local keys, windows, callbacks = {}, {}, {}
local original_filter = function() return nil end
local filters = {rules = {original_filter}, ewmh = {original_filter}}
local identity, readable = "1234", true
io.open = function(path, mode)
    assert(path == "/proc/42/stat" and mode == "r")
    if not readable then return nil end
    return {read = function(_, size)
        assert(size == 4096)
        return "42 (desktop with ) parens) S " .. string.rep("0 ", 18) .. identity .. " 0\n"
    end, close = function() end}
end
root = {keys = function(value) if value then keys = value end; return keys end}
client = {get = function() return windows end,
    connect_signal = function(name, callback) callbacks[name] = callback end,
    disconnect_signal = function(name, callback)
        assert(callbacks[name] == callback); callbacks[name] = nil
    end}
home_screen = {}
require = function(name)
    assert(name == "awful")
    return {ewmh = {
        add_activate_filter = function(callback, context) table.insert(filters[context], callback) end,
        remove_activate_filter = function(callback, context)
            assert(filters[context][2] == callback); table.remove(filters[context], 2)
        end}, key = function(modifiers, name, callback)
        return {{key = name, modifiers = modifiers, callback = callback}}
    end}
end
local function window(name, class, pid)
    local c = {valid = true, name = name, class = class, pid = pid, raised = 0}
    function c:raise() self.raised = self.raised + 1 end
    return c
end
'''
        script = fixture + '\n' + s.HOME_HOOK + r'''
assert(client.focus == nil)
local shell = window("Vitrallis", "vitrallis", 100)
callbacks.manage(shell)
assert(client.focus == shell)
-- A title alone is insufficient to identify the existing session desktop.
local impostor = window("pocket-home", nil, 42)
callbacks.manage(impostor)
assert(filters.ewmh[2](impostor, "ewmh") == nil)
local desktop = window("pocket-home", nil, 42)
home_screen.client = desktop
callbacks.manage(desktop)
assert(client.focus == shell and vitrallis_home_route.previous == desktop)
assert(filters.rules[2](desktop, "rules") == false)
assert(filters.ewmh[2](desktop, "ewmh") == false)
assert(filters.ewmh[2](desktop, "mouse.enter") == nil)
callbacks.focus(shell)
client.focus = desktop
callbacks.focus(desktop)
assert(client.focus == shell)
local app = window("Notepad", "vitrallis-notepad", 101)
client.focus = app
callbacks.focus(app)
assert(filters.ewmh[2](app, "ewmh") == nil)
callbacks["property::name"](shell)
assert(client.focus == app)
-- The original window can be destroyed before the same process maps a dialog.
desktop.valid = false
local dialog = window("Checking for updates", nil, 42)
assert(filters.rules[2](dialog, "rules") == false)
assert(filters.ewmh[2](dialog, "ewmh") == false)
client.focus = dialog
callbacks.focus(dialog)
assert(client.focus == app)
identity = "5678"
assert(filters.ewmh[2](dialog, "ewmh") == nil)
client.focus = dialog
callbacks.focus(dialog)
assert(client.focus == dialog)
client.focus = app
callbacks.focus(app)
identity = "1234"; readable = false
assert(filters.ewmh[2](dialog, "ewmh") == nil)
readable = true; shell.valid = false
assert(filters.ewmh[2](dialog, "ewmh") == nil)
shell.valid = true
local remapped = window("pocket-home", nil, 42)
home_screen.client = remapped
callbacks.manage(remapped)
assert(vitrallis_home_route.previous == remapped)
assert(client.focus == app)
app.valid = false
client.focus = remapped
callbacks.focus(remapped)
assert(client.focus == shell)
''' + '\n' + s.RESTORE_HOOK + r'''
assert(client.focus == remapped and remapped.raised == 1)
assert(vitrallis_home_route == nil and callbacks.manage == nil)
assert(callbacks.focus == nil)
assert(#filters.rules == 1 and filters.rules[1] == original_filter)
assert(#filters.ewmh == 1 and filters.ewmh[1] == original_filter)
'''
        script = script.replace(s.HOME_HOOK, 'do local status = (function()\n' + s.HOME_HOOK + '\nend)() end')
        script = script.replace(s.RESTORE_HOOK, 'do local status = (function()\n' + s.RESTORE_HOOK + '\nend)() end')
        result = subprocess.run([LUA, '-'], input=script, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_compositor_preserves_existing_owner_and_requires_present(self):
        x = Mock()
        x.XOpenDisplay.return_value = 1
        x.XDefaultScreen.return_value = 0
        x.XInternAtom.return_value = 42
        x.XGetSelectionOwner.return_value = 123
        with patch.object(s.ctypes, 'CDLL', return_value=x), patch.object(s, 'log_chunk') as log, \
                patch.object(s.subprocess, 'Popen') as spawn:
            self.assertIsNone(s.start_compositor(Path('/unused')))
            spawn.assert_not_called()
            self.assertIn(b'compositor=existing', log.call_args.args[1])
            x.XGetSelectionOwner.return_value = 0
            x.XQueryExtension.return_value = 0
            self.assertIsNone(s.start_compositor(Path('/unused')))
            spawn.assert_not_called()
            self.assertIn(b'X Present extension unavailable', log.call_args.args[1])
            x.XQueryExtension.return_value = 1
            self.assertIs(s.start_compositor(Path('/unused')), spawn.return_value)
            self.assertEqual(spawn.call_args.args[0],
                             ['/usr/bin/picom', '--config', '/dev/null', '--backend', 'xrender', '--vsync'])
        self.assertEqual(x.XCloseDisplay.call_count, 3)

    def test_session_drains_and_stops_only_its_compositor(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(s, 'awesome'):
            root = Path(temp).resolve()
            generation = root / 'generations' / ('a' * 64)
            generation.mkdir(parents=True)
            (root / 'current').symlink_to('generations/' + 'a' * 64)
            binary = generation / 'vitrallis'
            binary.write_text('#!/bin/sh\nsleep 0.2\necho shell-done\n')
            binary.chmod(0o755)
            compositor = subprocess.Popen(['/bin/sh', '-c', 'echo compositor-ready; exec sleep 30'],
                                          stdout=subprocess.PIPE)
            with patch.object(s, 'start_compositor', return_value=compositor):
                self.assertEqual(s.supervise(root), 0)
            self.assertIsNotNone(compositor.poll())
            self.assertTrue(compositor.stdout.closed)
            self.assertIn('compositor-ready', (root / 'session.log').read_text())

    def test_explicit_argv_and_cleanup_policy(self):
        env = dict(DISPLAY=':0', XAUTHORITY='/home/chip/.Xauthority', DBUS_SESSION_BUS_ADDRESS='unix:path=/run/user/1000/bus')
        args = s.launch_command(Path('/space here/100%/session.py'), env)
        self.assertIn('--property=KillMode=control-group', args)
        self.assertIn('--property=Restart=no', args)
        self.assertIn('--property=TimeoutStopSec=5', args)
        self.assertEqual(args[-2:], ['/space here/100%/session.py', 'run'])
        self.assertIn('--property=ExecStopPost=/usr/bin/python3 "/space here/100%%/session.py" restore', args)
        with self.assertRaises(ValueError):
            s.launch_command(Path('/session.py'), {})

    def test_log_growth_bounded_and_symlinks_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            log = root / 'session.log'
            for _ in range(50):
                s.log_chunk(log, b'x' * 8192)
            self.assertLessEqual(log.stat().st_size, s.LIMIT)
            self.assertLessEqual(log.with_suffix('.log.1').stat().st_size, s.LIMIT)
            log.unlink()
            log.symlink_to(root / 'outside')
            with self.assertRaises(ValueError):
                s.log_chunk(log, b'bad')
            self.assertFalse((root / 'outside').exists())

    def test_missing_binary_never_changes_home_routing(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(s, 'awesome') as awesome:
            with self.assertRaises(RuntimeError):
                s.supervise(Path(temp).resolve())
            awesome.assert_not_called()

    def test_hardlinked_log_and_dangling_marker_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            outside = root / 'outside'
            outside.write_bytes(b'preserve')
            log = root / 'session.log'
            os.link(outside, log)
            with self.assertRaisesRegex(ValueError, 'hardlink'):
                s.log_chunk(log, b'bad')
            self.assertEqual(outside.read_bytes(), b'preserve')
            (root / '.installation-pending').symlink_to(root / 'missing')
            with patch.object(s, 'awesome') as awesome:
                with self.assertRaisesRegex(ValueError, 'symlink'):
                    s.supervise(root)
                awesome.assert_not_called()

    def test_missing_companion_can_show_repair_diagnostic_but_escaped_pointer_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(s, 'awesome') as awesome:
            root = Path(temp).resolve()
            generation = root / 'generations' / ('a' * 64)
            generation.mkdir(parents=True)
            (root / 'current').symlink_to('generations/' + 'a' * 64)
            for name in ('vitrallis', 'vitrallis-terminal', 'vitrallis-notepad'):
                path = generation / name
                path.write_text('#!/bin/sh\nexit 0\n')
                path.chmod(0o755)
            self.assertEqual(s.supervise(root), 0)
            self.assertEqual([call.args[0] for call in awesome.call_args_list], [s.HOME_HOOK, s.RESTORE_HOOK])
            awesome.reset_mock()
            (root / 'current').unlink()
            (root / 'current').symlink_to('../../outside')
            with self.assertRaisesRegex(RuntimeError, 'Invalid native build pointer'):
                s.supervise(root)
            awesome.assert_not_called()

    def test_run_resolves_binary_beside_the_session_script(self):
        with tempfile.TemporaryDirectory() as temp:
            script = Path(temp).resolve() / 'installed session with spaces/vitrallis-session.py'
            with patch.object(s, '__file__', str(script)), \
                    patch.object(s.sys, 'argv', [str(script), 'run']), \
                    patch.object(s, 'supervise', return_value=17) as supervise:
                self.assertEqual(s.main(), 17)
                supervise.assert_called_once_with(script.parent)

    def test_stop_rechecks_identity_and_restores_bindings(self):
        script = Path('/home/chip/.local/share/vitrallis/vitrallis-session.py')
        with patch.object(s, 'owned_session', side_effect=[('12', '42'), ('12', '42'), None]), \
                patch.object(s.subprocess, 'run') as run, patch.object(s, 'awesome') as awesome:
            self.assertTrue(s.stop_owned(script))
            self.assertEqual(run.call_args.args[0], ['/usr/bin/systemctl', '--user', 'stop', 'vitrallis-session.service'])
            awesome.assert_called_once_with(s.RESTORE_HOOK)
        with patch.object(s, 'owned_session', side_effect=[('12', '42'), ('12', '43')]), \
                patch.object(s.subprocess, 'run') as run, self.assertRaisesRegex(ValueError, 'changed'):
            s.stop_owned(script)
        run.assert_not_called()
        with patch.object(s, 'owned_session', return_value=('12', '42')), patch.object(s.subprocess, 'run') as run:
            self.assertTrue(s.stop_owned(script, dry_run=True))
            run.assert_not_called()

    def test_launch_cannot_race_exclusive_update_or_removal(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            stage = base / '.vitrallis-update'
            stage.mkdir()
            with (stage / 'lock').open('w') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                with patch.object(s.subprocess, 'run') as run, self.assertRaises(BlockingIOError):
                    s.launch(base / 'vitrallis-session.py')
                run.assert_not_called()
            (stage / 'removal').mkdir()
            with patch.object(s, 'awesome') as awesome, self.assertRaisesRegex(RuntimeError, 'removal'):
                s.supervise(base)
            awesome.assert_not_called()

    def test_session_ownership_requires_exact_transient_unit_and_process(self):
        script = Path('/home/chip/.local/share/vitrallis/vitrallis-session.py')
        state = dict(LoadState='loaded', ActiveState='active', Transient='no', MainPID='23')
        with patch.object(s, 'unit_state', return_value=state), self.assertRaisesRegex(ValueError, 'unrelated'):
            s.owned_session(script)
        state['Transient'] = 'yes'
        with tempfile.TemporaryDirectory() as temporary:
            proc = Path(temporary).resolve()
            process = proc / '23'
            process.mkdir()
            (process / 'cmdline').write_bytes(b'/usr/bin/python3\0' + os.fsencode(script) + b'\0run\0')
            (process / 'stat').write_text('23 (python3) ' + ' '.join(['0'] * 19 + ['42']))
            with patch.object(s, 'unit_state', return_value=state), patch.object(s, 'Path', side_effect=lambda value: proc if value == '/proc' else Path(value)):
                self.assertEqual(s.owned_session(script), ('23', '42'))
                (process / 'cmdline').write_bytes(b'/usr/bin/python3\0unrelated.py\0')
                with self.assertRaisesRegex(ValueError, 'not owned'):
                    s.owned_session(script)
        with patch.object(s, 'unit_state', return_value={'LoadState': 'not-found'}):
            self.assertIsNone(s.owned_session(script))

    def test_run_wrapper_preserves_environment_arguments_cwd_and_exit_status(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            home = root / 'user home'
            launcher = home / '.local/share/vitrallis/launch'
            launcher.parent.mkdir(parents=True)
            launcher.write_text(
                '#!/bin/sh\n'
                'printf "%s\\n" "$HOME" "$PWD" "$DISPLAY" "$XAUTHORITY" "$@"\n'
                'exit 17\n'
            )
            launcher.chmod(0o755)
            package = root / 'package with spaces'
            package.mkdir()
            wrapper = package / 'run-session.sh'
            shutil.copyfile(Path(s.__file__).parent / 'run-session.sh', wrapper)
            cwd = root / 'unrelated directory'
            cwd.mkdir()
            args = ['argument with spaces', 'literal $HOME']
            result = subprocess.run(
                ['sh', str(wrapper), *args], cwd=cwd,
                env=dict(os.environ, HOME=str(home), DISPLAY=':91', XAUTHORITY='fixture auth'),
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 17, result.stderr)
            self.assertEqual(result.stdout.splitlines(), [str(home), str(cwd), ':91', 'fixture auth', *args])


if __name__ == '__main__':
    unittest.main()
