# -*- mode: python ; coding: utf-8 -*-

block_cipher = None

EXCLUDES = [
    'numpy', 'numpy.core', 'numpy.random', 'numpy.linalg', 'numpy.fft',
    'matplotlib', 'scipy', 'pandas', 'cv2', 'sklearn', 'numba',
    'setuptools', 'pkg_resources', 'pkg_resources._vendor',
    'pytest', 'unittest', 'pdb', 'pydoc', 'doctest', 'test',
    'IPython', 'jupyter', 'notebook',
    'tkinter.test', 'lib2to3', 'distutils',
    'PIL.ImageQt', 'PIL.AvifImagePlugin', 'PIL.WebPImagePlugin',
    'PIL.ImageCms', 'PIL.PdfImagePlugin', 'PIL.FpxImagePlugin',
    'PIL.DdsImagePlugin', 'PIL.BlpImagePlugin', 'PIL.McIdasImagePlugin',
    'PIL.MpegImagePlugin', 'PIL.SpiderImagePlugin', 'PIL.SunImagePlugin',
    'PIL.SgiImagePlugin', 'PIL.XbmImagePlugin', 'PIL.XpmImagePlugin',
    'PIL.XVThumbImagePlugin', 'PIL.GbrImagePlugin', 'PIL.Hdf5StubImagePlugin',
    'PIL.BufrStubImagePlugin', 'PIL.FtexImagePlugin', 'PIL.GribStubImagePlugin',
    'PIL.MicImagePlugin', 'PIL.MspImagePlugin', 'PIL.PalmImagePlugin',
    'PIL.PcdImagePlugin', 'PIL.PcxImagePlugin', 'PIL.PixarImagePlugin',
    'PIL.PpmImagePlugin', 'PIL.PsdImagePlugin', 'PIL.QoiImagePlugin',
    'PIL.TgaImagePlugin', 'PIL.WmfImagePlugin', 'PIL.IptcImagePlugin',
    'PIL.ImtImagePlugin', 'PIL.Hdf5StubImagePlugin',
]


def _norm(path):
    return (path or '').replace('\\', '/').lower()


def _keep_binary(item):
    dest = _norm(item[0])
    if dest.startswith('numpy') or '/numpy' in dest or dest.startswith('numpy.libs'):
        return False
    if 'openblas' in dest or 'libscipy_openblas' in dest:
        return False
    if any(x in dest for x in ('_avif', '_webp', '_imagingcms', '_imagingmath', '_imagingmorph')):
        return False
    if dest.startswith('setuptools') or 'pkg_resources' in dest:
        return False
    return True


def _keep_data(item):
    dest = _norm(item[0])
    if dest.startswith('_tcl_data/tzdata/') or '/tzdata/' in dest:
        return False
    if dest.startswith('_tcl_data/msgs/') or dest.startswith('_tk_data/msgs/'):
        return False
    if dest.startswith('_tk_data/demos/') or '/tk8.6/demos/' in dest:
        return False
    if dest.startswith('numpy') or dest.startswith('setuptools') or 'pkg_resources' in dest:
        return False
    return True


a = Analysis(
    ['halit_changer.py'],
    pathex=[],
    binaries=[],
    datas=[('assets', 'assets'), ('skin_ids.json', '.')],
    hiddenimports=[],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=EXCLUDES,
    noarchive=False,
    optimize=2,
)
a.binaries = [b for b in a.binaries if _keep_binary(b)]
a.datas = [d for d in a.datas if _keep_data(d)]

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='Halit Changer',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=['assets\\icon.ico'],
)
