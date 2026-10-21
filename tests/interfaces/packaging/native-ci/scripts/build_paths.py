"""Omit local compiler path prefixes from newly qualified native artifacts."""
import os
from pathlib import Path


def build_environment(source: Path, work: Path) -> dict:
    env = dict(os.environ, CARGO_TARGET_DIR=str(work / 'target'))
    if env.get('RUSTFLAGS') or env.get('CARGO_ENCODED_RUSTFLAGS'):
        raise ValueError('Native qualification requires unset inherited compiler flags')
    home = Path.home()
    roots = [(home, '/build-home'),
             (Path(env.get('CARGO_HOME', home / '.cargo')), '/cargo'),
             (Path(env.get('RUSTUP_HOME', home / '.rustup')), '/rustup'),
             (source, '/svm-replay')]
    # Cargo's encoded form preserves spaces in Windows or Unix checkout paths.
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(
        f'--remap-path-prefix={path.resolve()}={public}' for path, public in roots)
    return env
