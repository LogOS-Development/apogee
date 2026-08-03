import matplotlib.pyplot as plt
#!/usr/bin/env python3
"""Validate Apogee's JB2008 implementation against pyatmos."""

import subprocess
import sys
import types
from pathlib import Path

import numpy as np

# Make pyatmos importable without installing it (we downloaded the wheel).
PYATMOS_DIR = Path("/tmp/pyatmos_extract")
if not PYATMOS_DIR.exists():
    raise FileNotFoundError(
        "pyatmos wheel not extracted; run the download step first"
    )
sys.path.insert(0, str(PYATMOS_DIR))

# Stub optional dependencies that pyatmos requires but that are not needed
# for the pure JB2008 sub-function.
numba_stub = types.ModuleType("numba")
numba_stub.jit = lambda *args, **kwargs: (lambda f: f)
sys.modules["numba"] = numba_stub

wget_stub = types.ModuleType("wget")
wget_stub.download = lambda *args, **kwargs: None
sys.modules["wget"] = wget_stub

pyatmos_pkg = types.ModuleType("pyatmos")
pyatmos_pkg.__path__ = [str(PYATMOS_DIR / "pyatmos")]
sys.modules["pyatmos"] = pyatmos_pkg

from pyatmos.jb2008.JB2008_subfunc import JB2008  # type: ignore[import]
from pyatmos.jb2008.spaceweather import get_sw, read_sw_jb2008  # type: ignore[import]

# Shared validation framework
SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
from validation_framework import ComparisonPlot, Source, SummaryWriter

# Reference space-weather files (downloaded ahead of time)
SW_DIR = Path("/tmp/jb2008_sw")
SW_FILES = [SW_DIR / "SOLFSMY.TXT", SW_DIR / "DTCFILE.TXT"]


def load_apogee_csv() -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    cmd = "cargo run --example jb2008_samples -p apogee-core --release"
    result = subprocess.run(cmd, shell=True, cwd=SCRIPT_DIR.parent, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"jb2008_samples failed:\n{result.stderr}")
    lines = [ln for ln in result.stdout.strip().split("\n") if not ln.startswith("altitude")]
    alts, rhos, texos, tlocs = [], [], [], []
    for ln in lines:
        a, r, t, l = ln.split(",")
        alts.append(float(a))
        rhos.append(float(r))
        texos.append(float(t))
        tlocs.append(float(l))
    return np.array(alts), np.array(rhos), np.array(texos), np.array(tlocs)


def reference_values(
    alts: np.ndarray,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    swdata = read_sw_jb2008([str(p) for p in SW_FILES])

    # Reference epoch: 2014-07-22 22:18:45 UTC; fixed geometry (Sun RA/Dec = 0)
    import erfa

    t_year, t_doy, t_hour, t_min, t_sec = 2014, 203, 22, 18, 45
    dj, df = erfa.dtf2d("UTC", t_year, 7, 22, t_hour, t_min, t_sec)
    mjd = float(dj) + float(df) - 2400000.5

    f10, f10b, s10, s10b, m10, m10b, y10, y10b, dtc = get_sw(swdata, mjd)

    # Day of year with fraction for the reference time.
    yday = t_doy + (t_hour * 3600 + t_min * 60 + t_sec) / 86400.0

    rhos = np.empty_like(alts)
    texos = np.empty_like(alts)
    tlocs = np.empty_like(alts)

    for i, alt in enumerate(alts):
        temp, rho = JB2008(
            mjd,
            yday,
            (0.0, 0.0),
            (0.0, np.radians(25.0), alt),
            f10,
            f10b,
            s10,
            s10b,
            m10,
            m10b,
            y10,
            y10b,
            dtc,
        )
        rhos[i] = rho
        texos[i] = temp[0]
        tlocs[i] = temp[1]

    return rhos, texos, tlocs


def main() -> None:
    out_dir = Path("plots/jb2008")
    out_dir.mkdir(parents=True, exist_ok=True)

    alts, rho_apogee, texo_apogee, tloc_apogee = load_apogee_csv()
    rho_ref, texo_ref, tloc_ref = reference_values(alts)

    rho_src_a = Source.from_arrays("Apogee", alts, rho_apogee, unit="kg/m³")
    rho_src_r = Source.from_arrays("pyatmos", alts, rho_ref, unit="kg/m³")
    texo_src_a = Source.from_arrays("Apogee", alts, texo_apogee, unit="K")
    texo_src_r = Source.from_arrays("pyatmos", alts, texo_ref, unit="K")
    tloc_src_a = Source.from_arrays("Apogee", alts, tloc_apogee, unit="K")
    tloc_src_r = Source.from_arrays("pyatmos", alts, tloc_ref, unit="K")

    plot = ComparisonPlot(title="JB2008 total mass density: Apogee vs pyatmos")
    plot.add_series(rho_src_r, rho_src_a, name="density", ylabel="ρ (kg/m³)", tolerance=0.05)
    fig = plot.build()
    fig.savefig(out_dir / "density.png", dpi=150, bbox_inches="tight")
    plt.close(fig)

    plot = ComparisonPlot(title="JB2008 exospheric temperature")
    plot.add_series(texo_src_r, texo_src_a, name="Texo", ylabel="T (K)", tolerance=1.0)
    fig = plot.build()
    fig.savefig(out_dir / "temperature_exo.png", dpi=150, bbox_inches="tight")
    plt.close(fig)

    plot = ComparisonPlot(title="JB2008 local temperature")
    plot.add_series(tloc_src_r, tloc_src_a, name="Tloc", ylabel="T (K)", tolerance=5.0)
    fig = plot.build()
    fig.savefig(out_dir / "temperature_local.png", dpi=150, bbox_inches="tight")
    plt.close(fig)

    # Metrics
    rel_rho_err = np.abs(rho_apogee - rho_ref) / rho_ref
    max_rel_rho = float(np.max(rel_rho_err))
    max_abs_texo = float(np.max(np.abs(texo_apogee - texo_ref)))
    max_abs_tloc = float(np.max(np.abs(tloc_apogee - tloc_ref)))

    SummaryWriter(out_dir / "summary.json").write(
        name="jb2008",
        passed=max_rel_rho < 0.05,  # 5% density tolerance
        max_residual=max_rel_rho,
        notes=[
            f"samples: {len(alts)} ({alts.min():.0f}–{alts.max():.0f} km)",
            f"max relative density error: {max_rel_rho:.3%}",
            f"max exo temperature diff: {max_abs_texo:.3f} K",
            f"max local temperature diff: {max_abs_tloc:.3f} K",
        ],
    )

    print(f"Plots saved to {out_dir}")
    print(f"  max relative density error: {max_rel_rho:.3%}")
    print(f"  max exo T diff: {max_abs_texo:.3f} K")
    print(f"  max local T diff: {max_abs_tloc:.3f} K")


if __name__ == "__main__":
    main()
