"""Create the player ZIP from a checked Windows build. Python is build-only."""
from pathlib import Path
import hashlib
import json
import subprocess
import tomllib
import zipfile


def package():
    root = Path(__file__).resolve().parents[1]
    version = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    files = {}
    for name in ('stalkshift.exe', 'stalkshift_plugin.dll'):
        files[name] = (root / 'target/release' / name).read_bytes()
    for folder in ('release', 'docs', 'fixtures', 'third-party'):
        for path in sorted((root / folder).rglob('*')):
            if path.is_file():
                target = path.relative_to(root / 'release') if folder == 'release' else path.relative_to(root)
                files[target.as_posix()] = path.read_bytes()
    for name in ('README.md', 'README.ru.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md'):
        files[name] = (root / name).read_bytes()
    # Carry dependency copyright/permission notices with the distributed binaries.
    metadata = json.loads(subprocess.check_output([
        'cargo', 'metadata', '--locked', '--format-version', '1',
        '--filter-platform', 'x86_64-pc-windows-msvc'], cwd=root))
    inventory = []
    for dependency in metadata['packages']:
        if not dependency['source']:
            continue
        directory = Path(dependency['manifest_path']).parent
        notices = [path for path in directory.iterdir() if path.is_file()
                   and path.name.lower().startswith(('license', 'copying', 'notice', 'unlicense'))]
        if not notices:
            raise RuntimeError(f"Missing dependency license: {dependency['name']}")
        prefix = f"third-party/dependencies/{dependency['name']}-{dependency['version']}"
        for path in notices:
            files[f'{prefix}/{path.name}'] = path.read_bytes()
        inventory.append({key: dependency[key] for key in ('name', 'version', 'license', 'repository')})
    files['third-party/dependencies.json'] = (json.dumps(inventory, indent=2) + '\n').encode()
    manifest = {'version': version, 'commit': commit, 'protocol': 3,
                'files': {name: hashlib.sha256(files[name]).hexdigest() for name in ('stalkshift.exe', 'stalkshift_plugin.dll')}}
    files['version.json'] = (json.dumps(manifest, indent=2) + '\n').encode()
    output = root / 'target/dist'
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f'StalkShift-{version}-windows-x64.zip'
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as bundle:
        for name, data in sorted(files.items()):
            entry = zipfile.ZipInfo(name, (2026, 9, 5, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            bundle.writestr(entry, data)
    with zipfile.ZipFile(archive) as bundle:
        assert bundle.testzip() is None
        assert set(bundle.namelist()) == set(files)
        for name, digest in manifest['files'].items():
            assert hashlib.sha256(bundle.read(name)).hexdigest() == digest
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (output / 'SHA256SUMS.txt').write_text(f'{digest}  {archive.name}\n', encoding='ascii')
    print(archive)
    print(digest)
    return archive


if __name__ == '__main__':
    package()
