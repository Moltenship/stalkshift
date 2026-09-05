"""Exercise the shipped setup script against isolated fake game/profile folders."""
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
    game = work / 'fake game & library'
    plugins = game / 'bin/win_x64/plugins'
    plugins.mkdir(parents=True)
    (plugins.parent / 'eurotrucks2.exe').write_bytes(b'fake game marker')
    other = plugins / 'unrelated_plugin.dll'
    other.write_bytes(b'preserve another plugin')
    destination = plugins / 'stalkshift_plugin.dll'
    destination.write_bytes(b'old StalkShift build')
    profile = work / 'profile/controls.sii'
    profile.parent.mkdir()
    original = b'''SiiNunit
{
device joy5 `di8.'{DEVICE}|{0024346E-0000-0000-0000-504944564944}'`
mix cruiectrl `keyboard.c?0 | joy5.b25?0 | joy.b7?0 | semantical.cruiectrl?0`
}
'''
    profile.write_bytes(original)
    profile.chmod(0o444)
    data = work / 'app data'

    def run(action, language='en', selected=profile, success=True):
        command = ['powershell.exe', '-NoLogo', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
                   str(package / 'setup/Setup.ps1'), '-Action', action, '-Language', language,
                   '-GameDirectory', str(game), '-DataDirectory', str(data), '-NonInteractive']
        if selected is not None:
            command += ['-ProfilePath', str(selected)]
        result = subprocess.run(command, capture_output=True, timeout=30)
        assert (result.returncode == 0) == success, result.stdout.decode('utf-8', errors='replace') + result.stderr.decode('utf-8', errors='replace')
        assert other.read_bytes() == b'preserve another plugin'

    run('Install')
    assert hashlib.sha256(destination.read_bytes()).hexdigest() == manifest['files']['stalkshift_plugin.dll']
    modified = original.replace(b'joy5.b25?0', b'unbound?0')
    assert profile.read_bytes() == modified
    assert profile.stat().st_file_attributes & 1, 'read-only attribute was lost'
    record = json.loads((data / 'install.json').read_text(encoding='utf-8'))
    assert Path(record['Profiles'][0]['Backup']).read_bytes() == original
    run('Install', language='ru')
    run('Uninstall', language='ru')
    assert not destination.exists()
    assert profile.read_bytes() == original
    assert profile.stat().st_file_attributes & 1
    # If bindings changed after installation, uninstall must not overwrite them.
    run('Install')
    profile.chmod(0o666)
    user_edit = profile.read_bytes().replace(b'keyboard.c', b'keyboard.v')
    profile.write_bytes(user_edit)
    run('Uninstall')
    assert profile.read_bytes() == user_edit
    # A failure after DLL replacement restores the old DLL.
    destination.write_bytes(b'rollback target')
    invalid = work / 'wrong-name.sii'
    invalid.write_bytes(original)
    run('Install', selected=invalid, success=False)
    assert destination.read_bytes() == b'rollback target'
    # Corrupt packages must fail before touching game files.
    (package / 'stalkshift_plugin.dll').write_bytes(b'corrupt')
    run('Install', success=False)
    assert destination.read_bytes() == b'rollback target'
    print('PASS: EN/RU install, upgrade, uninstall, read-only profile, user edits, rollback, corrupt package')


if __name__ == '__main__':
    main(Path(sys.argv[1]).resolve())
