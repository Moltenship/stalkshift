"""Exercise the player installer in isolated ETS2/ATS folders; no games needed."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tempfile
import zipfile


def main(archive):
    root = Path(__file__).resolve().parents[1]
    work = Path(tempfile.mkdtemp(prefix='installer-test-', dir=root / 'target'))
    package = work / 'package with spaces'
    with zipfile.ZipFile(archive) as bundle:
        bundle.extractall(package)
    manifest = json.loads((package / 'version.json').read_text(encoding='utf-8'))
    assert set(json.loads((package / 'setup/en.json').read_text(encoding='utf-8'))) == set(json.loads((package / 'setup/ru.json').read_text(encoding='utf-8')))
    original = b'''SiiNunit
{
device joy5 `di8.'{DEVICE}|{0024346E-0000-0000-0000-504944564944}'`
mix cruiectrl `keyboard.c?0 | joy5.b25?0 | joy.b7?0 | semantical.cruiectrl?0`
}
'''
    modified = original.replace(b'joy5.b25?0', b'unbound?0')
    data = work / 'app data'
    games = {}
    for kind, executable in [('ets2', 'eurotrucks2.exe'), ('ats', 'amtrucks.exe')]:
        game = work / f'{kind} game & library'
        plugins = game / 'bin/win_x64/plugins'
        plugins.mkdir(parents=True)
        (plugins.parent / executable).write_bytes(b'fake game marker')
        (plugins / 'unrelated_plugin.dll').write_bytes(b'preserve another plugin')
        profile = work / kind / 'controls.sii'
        profile.parent.mkdir()
        profile.write_bytes(original)
        profile.chmod(0o444)
        games[kind] = (game, plugins / 'stalkshift_plugin.dll', profile,
                       plugins / 'stalkshift-cruise-unit.txt')

    def record(kind):
        return json.loads((data / f'install-{kind}.json').read_text(encoding='utf-8'))

    def run(action, kind='ets2', language='en', selected=True, success=True,
            unit=None, explicit_kind=False, explicit_path=True, interactive=False, answer=None):
        game, _, profile, _ = games[kind]
        command = ['powershell.exe', '-NoLogo', '-NoProfile', '-ExecutionPolicy', 'Bypass',
                   '-File', str(package / 'setup/Setup.ps1'), '-Action', action,
                   '-Language', language, '-DataDirectory', str(data)]
        if not interactive:
            command += ['-NonInteractive']
        if explicit_path:
            command += ['-GameDirectory', str(game)]
        if explicit_kind:
            command += ['-Game', kind]
        if selected:
            command += ['-ProfilePath', str(profile if selected is True else selected)]
        if unit:
            command += ['-CruiseUnit', unit]
        result = subprocess.run(command, input=answer, capture_output=True, timeout=30)
        assert (result.returncode == 0) == success, result.stdout.decode('utf-8', errors='replace') + result.stderr.decode('utf-8', errors='replace')
        for game, _, _, _ in games.values():
            assert (game / 'bin/win_x64/plugins/unrelated_plugin.dll').read_bytes() == b'preserve another plugin'

    # Default units differ, while both games can be installed at once.
    run('Install')
    run('Install', 'ats', language='ru')
    for kind, unit in [('ets2', 'kmh'), ('ats', 'mph')]:
        _, dll, profile, settings = games[kind]
        assert hashlib.sha256(dll.read_bytes()).hexdigest() == manifest['files']['stalkshift_plugin.dll']
        assert profile.read_bytes() == modified and profile.stat().st_file_attributes & 1
        assert settings.read_text().strip() == unit
        assert Path(record(kind)['Profiles'][0]['Backup']).read_bytes() == original

    ets_record = (data / 'install-ets2.json').read_bytes()
    run('Install', 'ats', unit='kmh', explicit_kind=True, explicit_path=False)
    assert games['ats'][3].read_text().strip() == 'kmh'
    # With both games installed, interactive removal must affect the choice only.
    run('Uninstall', 'ats', interactive=True, explicit_path=False, selected=False, answer=b'2\n')
    assert (data / 'install-ets2.json').read_bytes() == ets_record
    assert games['ets2'][1].exists() and games['ets2'][2].read_bytes() == modified
    assert not games['ats'][1].exists() and not games['ats'][3].exists()
    assert games['ats'][2].read_bytes() == original

    # Migrate the actual shape of a 1.0 installation without losing its backup.
    legacy = record('ets2')
    legacy.pop('Settings')
    legacy.pop('Game')
    (data / 'install.json').write_text(json.dumps(legacy), encoding='utf-8')
    (data / 'install-ets2.json').unlink()
    games['ets2'][3].unlink()
    run('Install', language='ru', unit='mph')
    assert not json.loads((data / 'install.json').read_text(encoding='utf-8'))['Installed']
    assert record('ets2')['Profiles'] == legacy['Profiles']
    assert games['ets2'][3].read_text().strip() == 'mph'
    run('Install')
    assert games['ets2'][3].read_text().strip() == 'mph', 'reinstall lost chosen units'
    run('Uninstall')
    assert games['ets2'][2].read_bytes() == original
    assert games['ets2'][2].stat().st_file_attributes & 1

    # Edited profiles/settings are preserved on removal in either game.
    for kind in games:
        run('Install', kind)
        _, dll, profile, settings = games[kind]
        profile.chmod(0o666)
        edited = profile.read_bytes().replace(b'keyboard.c', b'keyboard.v')
        profile.write_bytes(edited)
        settings.write_bytes(b'custom user setting\n')
        run('Uninstall', kind)
        assert not dll.exists() and profile.read_bytes() == edited
        assert settings.read_bytes() == b'custom user setting\n'
        # Failure after replacement restores both the previous DLL and settings.
        dll.write_bytes(b'rollback target')
        invalid = work / f'{kind}-wrong-name.sii'
        invalid.write_bytes(original)
        run('Install', kind, selected=invalid, success=False)
        assert dll.read_bytes() == b'rollback target'
        assert settings.read_bytes() == b'custom user setting\n'
        # An existing settings file is backed up and restored on clean removal.
        run('Install', kind)
        run('Uninstall', kind)
        assert settings.read_bytes() == b'custom user setting\n'

    (package / 'stalkshift_plugin.dll').write_bytes(b'corrupt')
    for kind in games:
        run('Install', kind, success=False)
        assert not games[kind][1].exists()
    print('PASS: ETS2/ATS install, independent removal, selection, units, legacy migration, read-only profiles, user edits, rollback, corrupt package')


if __name__ == '__main__':
    main(Path(sys.argv[1]).resolve())
